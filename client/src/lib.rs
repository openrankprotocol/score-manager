mod sol;

use serde::{Deserialize, Serialize};
use std::{error::Error, str::FromStr};
use tracing::info;

use alloy::{
    hex,
    network::EthereumWallet,
    primitives::Address,
    providers::ProviderBuilder,
    signers::{k256::ecdsa::SigningKey, local::LocalSigner, Signer},
    transports::http::reqwest::Url,
};
use dotenv::dotenv;
use eyre::Result;

use openrank_common::{
    config,
    db::{self, Db, DbItem},
    tx::{consts, Body, Tx},
};
use sol::ComputeManager::{self, Signature};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub contract_address: String,
    pub rpc_url: String,
    pub database: db::Config,
    pub chain_id: u64,
}

pub struct ComputeManagerClient {
    contract_address: Address,
    rpc_url: Url,
    signer: LocalSigner<SigningKey>,
    db: Db,
}

impl ComputeManagerClient {
    pub fn init() -> Result<Self, Box<dyn Error>> {
        dotenv().ok();
        let secret_key_hex = std::env::var("SC_CLIENT_WALLET_SECRET_KEY")?;
        let secret_key_bytes = hex::decode(secret_key_hex)?;
        let secret_key = SigningKey::from_slice(secret_key_bytes.as_slice())?;

        let config_loader = config::Loader::new("openrank-smart-contract-client")?;
        let config: Config = config_loader.load_or_create(include_str!("../config.toml"))?;

        let contract_address = Address::from_str(&config.contract_address)?;
        let rpc_url = Url::parse(&config.rpc_url)?;
        let mut signer: LocalSigner<SigningKey> = secret_key.into();
        signer.set_chain_id(Some(config.chain_id));
        let db = Db::new_secondary(&config.database, &[&Tx::get_cf()])?;
        let client = Self::new(contract_address, rpc_url, signer, db);
        Ok(client)
    }

    pub fn new(
        contract_address: Address,
        rpc_url: Url,
        signer: LocalSigner<SigningKey>,
        db: Db,
    ) -> Self {
        Self {
            contract_address,
            rpc_url,
            signer,
            db,
        }
    }

    pub async fn submit_openrank_tx(&self, tx: Tx) -> Result<()> {
        // create a contract ins(body)tance
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.clone());
        let contract = ComputeManager::new(self.contract_address, provider);

        // check if tx already exists
        let is_tx_exists = match tx.body() {
            Body::ComputeCommitment(_) | Body::ComputeVerification(_) => {
                contract.hasTx(tx.hash().0.into()).call().await?._0
            }
            _ => true,
        };
        if is_tx_exists {
            return Ok(());
        }

        // submit tx
        let _result_hash = match tx.body() {
            Body::ComputeCommitment(body) => {
                let compute_commitment = body;
                let compute_assignment_tx_hash = compute_commitment.assignment_tx_hash.0.into();
                let compute_commitment_tx_hash = tx.hash().0.into();
                let compute_root_hash = compute_commitment.compute_root_hash.0.into();
                let sig = Signature {
                    s: tx.signature().s.into(),
                    r: tx.signature().r.into(),
                    r_id: tx.signature().r_id(),
                };
                contract
                    .submitComputeCommitment(
                        compute_assignment_tx_hash,
                        compute_commitment_tx_hash,
                        compute_root_hash,
                        sig,
                    )
                    .send()
                    .await?
                    .watch()
                    .await?
            }
            Body::ComputeVerification(body) => {
                let compute_verification = body;
                let compute_verification_tx_hash = tx.hash().0.into();
                let compute_assignment_tx_hash = compute_verification.assignment_tx_hash.0.into();
                let sig = Signature {
                    s: tx.signature().s.into(),
                    r: tx.signature().r.into(),
                    r_id: tx.signature().r_id(),
                };
                contract
                    .submitComputeVerification(
                        compute_verification_tx_hash,
                        compute_assignment_tx_hash,
                        sig,
                    )
                    .send()
                    .await?
                    .watch()
                    .await?
            }
            _ => return Ok(()),
        };

        Ok(())
    }

    fn read_txs(&self) -> Result<Vec<Tx>> {
        // collect all txs
        let mut txs = Vec::new();

        let mut compute_commitment_txs: Vec<Tx> = self
            .db
            .read_from_end(consts::COMPUTE_COMMITMENT, None)
            .map_err(|e| eyre::eyre!(e))?;
        txs.append(&mut compute_commitment_txs);
        drop(compute_commitment_txs);

        let mut compute_verification_txs: Vec<Tx> = self
            .db
            .read_from_end(consts::COMPUTE_VERIFICATION, None)
            .map_err(|e| eyre::eyre!(e))?;
        txs.append(&mut compute_verification_txs);
        drop(compute_verification_txs);

        Ok(txs)
    }

    pub async fn run(&self) -> Result<()> {
        // Sync up the db first
        self.db.refresh()?;
        let txs = self.read_txs()?;
        for tx in txs {
            self.submit_openrank_tx(tx.clone()).await?;
            info!("Posted TX on chain: {:?}", tx.hash());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloy::{
        network::EthereumWallet, node_bindings::Anvil, primitives::address,
        signers::local::PrivateKeySigner,
    };

    use openrank_common::{
        merkle::Hash,
        tx::{
            compute::{Commitment, Request, Verification},
            TxHash,
        },
    };

    use super::*;

    fn config_for_dir(directory: &str) -> db::Config {
        db::Config {
            directory: directory.to_string(),
            secondary: None,
        }
    }

    #[tokio::test]
    async fn test_submit_openrank_tx() -> Result<()> {
        let test_mnemonic = String::from(
            "work man father plunge mystery proud hollow address reunion sauce theory bonus",
        );

        // Spin up a local Anvil node.
        // Ensure `anvil` is available in $PATH.
        let anvil = Anvil::new().mnemonic(&test_mnemonic).try_spawn()?;

        // Set up signer from the first default Anvil account (Alice).
        let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
        let wallet = EthereumWallet::from(signer.clone());

        // Create a provider with the wallet.
        let rpc_url: Url = anvil.endpoint().parse()?;
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(rpc_url.clone());

        // Deploy the `ComputeManager` contract.
        let submitters = vec![signer.address()];
        let computers = vec![address!("13978aee95f38490e9769c39b2773ed763d9cd5f")];
        let verifiers = vec![address!("cd2a3d9f938e13cd947ec05abc7fe734df8dd826")];
        let contract = ComputeManager::deploy(&provider, submitters, computers, verifiers).await?;

        // Create a contract instance.
        let contract_address = *contract.address();
        let db = Db::new(&config_for_dir("test-pg-storage"), &[&Tx::get_cf()]).unwrap();
        let client = ComputeManagerClient::new(contract_address, rpc_url, signer, db);

        // Try to submit "ComputeRequest" TX
        client
            .submit_openrank_tx(Tx::default_with(Body::ComputeRequest(Request::default())))
            .await?;

        // Try to submit "ComputeCommitment" TX
        let sk_bytes_hex = "c87f65ff3f271bf5dc8643484f66b200109caffe4bf98c4cb393dc35740b28c0";
        let sk_bytes = hex::decode(sk_bytes_hex).unwrap();
        let sk = SigningKey::from_slice(&sk_bytes).unwrap();
        let mut tx = Tx::default_with(Body::ComputeCommitment(Commitment::new(
            TxHash::from_bytes(
                hex::decode("43924aa0eb3f5df644b1d3b7d755190840d44d7b89f1df471280d4f1d957c819")
                    .unwrap(),
            ),
            Hash::default(),
            Hash::default(),
            vec![],
        )));
        let _ = tx.sign(&sk);

        client.submit_openrank_tx(tx).await?;

        // Try to submit "ComputeVerification" TX
        let sk_bytes_hex = "c85ef7d79691fe79573b1a7064c19c1a9819ebdbd1faaab1a8ec92344438aaf4";
        let sk_bytes = hex::decode(sk_bytes_hex).unwrap();
        let sk = SigningKey::from_slice(&sk_bytes).unwrap();

        let mut tx = Tx::default_with(Body::ComputeVerification(Verification::new(
            TxHash::from_bytes(
                hex::decode("43924aa0eb3f5df644b1d3b7d755190840d44d7b89f1df471280d4f1d957c819")
                    .unwrap(),
            ),
            true,
        )));
        let _ = tx.sign(&sk);

        client.submit_openrank_tx(tx).await?;

        Ok(())
    }
}
