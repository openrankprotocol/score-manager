mod sol;

use jsonrpsee::{core::client::ClientT, http_client::HttpClient};
use log::{debug, info};
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::{error::Error, str::FromStr, sync::Arc, time::Duration};

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
    tx::{compute, Body, Tx, TxHash},
};
use sol::ComputeManager::{self, Signature};

const COUNTER_KEY: &str = "seq_number";
const FAILED_SEQ_NUMBERS_KEY: &str = "failed_seq_numbers";

const BATCH_SIZE: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub contract_address: String,
    pub chain_rpc_url: String,
    pub chain_id: u64,
    pub openrank_rpc_url: String,
    pub db_path: String,
    pub interval_seconds: u64,
}

#[derive(Debug)]
pub struct Db {
    connection: DB,
    db_path: String,
}

enum SubmitSingleComputeResultResult {
    Submitted,
    NotVerified(u64), // refers to `seq_number` whose result has empty verification txs
}

pub struct ComputeManagerClient {
    contract_address: Address,
    chain_rpc_url: Url,
    signer: LocalSigner<SigningKey>,
    openrank_rpc_url: String,
    db: Arc<Db>,
    interval_seconds: u64,
}

impl ComputeManagerClient {
    pub fn init() -> Result<Self, Box<dyn Error>> {
        dotenv().ok();
        let secret_key_hex = std::env::var("SUBMITTER_SECRET_KEY")?;
        let secret_key_bytes = hex::decode(secret_key_hex)?;
        let secret_key = SigningKey::from_slice(secret_key_bytes.as_slice())?;

        let config_loader = config::Loader::new("openrank-smart-contract-client")?;
        let config: Config = config_loader.load_or_create(include_str!("../config.toml"))?;

        let contract_address = Address::from_str(&config.contract_address)?;
        let chain_rpc_url = Url::parse(&config.chain_rpc_url)?;
        let mut signer: LocalSigner<SigningKey> = secret_key.into();
        signer.set_chain_id(Some(config.chain_id));

        let connection = DB::open_default(&config.db_path)?;
        let db = Db {
            connection,
            db_path: config.db_path,
        };

        let client = Self::new(
            contract_address,
            chain_rpc_url,
            signer,
            config.openrank_rpc_url,
            db,
            config.interval_seconds,
        );
        Ok(client)
    }

    pub fn new(
        contract_address: Address,
        chain_rpc_url: Url,
        signer: LocalSigner<SigningKey>,
        openrank_rpc_url: String,
        db: Db,
        interval_seconds: u64,
    ) -> Self {
        Self {
            contract_address,
            chain_rpc_url,
            signer,
            openrank_rpc_url,
            db: Arc::new(db),
            interval_seconds,
        }
    }

    /// Submit the single openrank TX to on-chain smart contract
    pub async fn submit_openrank_tx(&self, tx: Tx) -> Result<()> {
        // create a contract instance
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.chain_rpc_url.clone());
        let contract = ComputeManager::new(self.contract_address, provider);

        // check if tx already exists
        let is_tx_exists = match tx.body() {
            Body::ComputeCommitment(_) | Body::ComputeVerification(_) => {
                contract.hasTx(tx.hash().inner().into()).call().await?._0
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
                let compute_assignment_tx_hash =
                    compute_commitment.assignment_tx_hash().inner().into();
                let compute_commitment_tx_hash = tx.hash().inner().into();
                let compute_root_hash = compute_commitment.compute_root_hash().inner().into();
                let sig = Signature {
                    s: tx.signature().s().into(),
                    r: tx.signature().r().into(),
                    r_id: *tx.signature().r_id(),
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
                let compute_verification_tx_hash = tx.hash().inner().into();
                let compute_assignment_tx_hash =
                    compute_verification.assignment_tx_hash().inner().into();
                let sig = Signature {
                    s: tx.signature().s().into(),
                    r: tx.signature().r().into(),
                    r_id: *tx.signature().r_id(),
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

    /// Fetch multiple openrank TXs
    pub async fn fetch_openrank_txs(&self, txs_arg: Vec<(String, TxHash)>) -> Result<Vec<Tx>> {
        // Creates a new client
        let client = HttpClient::builder().build(self.openrank_rpc_url.as_str())?;

        // fetch txs
        let txs = client.request("sequencer_get_txs", vec![txs_arg]).await?;

        Ok(txs)
    }

    /// Fetch single openrank TX
    pub async fn fetch_openrank_tx(&self, prefix: String, tx_hash: TxHash) -> Result<Tx> {
        // Creates a new client
        let client = HttpClient::builder().build(self.openrank_rpc_url.as_str())?;

        // fetch tx
        let tx = client
            .request("sequencer_get_tx", (prefix, tx_hash))
            .await?;

        Ok(tx)
    }

    /// Fetch single openrank compute result
    async fn fetch_openrank_compute_result(&self, seq_number: u64) -> Result<compute::Result> {
        // Creates a new client
        let client = HttpClient::builder().build(self.openrank_rpc_url.as_str())?;

        // fetch compute result
        let compute_result = client
            .request("sequencer_get_compute_result", vec![seq_number])
            .await?;

        Ok(compute_result)
    }

    /// Submit the TXs of multiple OpenRank compute results
    async fn submit_compute_result_txs(&self) -> Result<(), Box<dyn Error>> {
        // fetch the last `seq_number`
        let last_seq_number = self.retrieve_seq_number().await?;
        let mut curr_seq_number = last_seq_number;

        for _ in 0..BATCH_SIZE {
            info!(
                "submitting compute result for seq_number: {:?}",
                curr_seq_number
            );

            let res = self
                .submit_single_compute_result_txs(curr_seq_number)
                .await?;

            if let SubmitSingleComputeResultResult::NotVerified(seq_number) = res {
                self.append_failed_seq_number(seq_number).await?
            };

            // increment & save the `seq_number`
            curr_seq_number += 1;
            self.store_seq_number(curr_seq_number).await?;
        }

        Ok(())
    }

    /// Try to submit the TXs of the `failed_seq_numbers` & remove `seq_number` if succeed.
    async fn retry_failed_seq_numbers(&self) -> Result<(), Box<dyn Error>> {
        let failed_seq_numbers = self.retrieve_failed_seq_numbers().await?;
        for seq_number in failed_seq_numbers {
            let res = self.submit_single_compute_result_txs(seq_number).await?;
            if let SubmitSingleComputeResultResult::Submitted = res {
                self.remove_failed_seq_number(seq_number).await?
            };
        }
        Ok(())
    }

    /// Submit the openrank TX into on-chain smart contract, in periodic interval
    pub async fn start_interval_submit(&self) -> Result<(), Box<dyn Error>> {
        let mut interval = tokio::time::interval(Duration::from_secs(self.interval_seconds));

        loop {
            interval.tick().await;
            info!("Running periodic submission...");
            let res = self.submit_compute_result_txs().await;
            if let Err(e) = res {
                debug!("Submittion error: {:?}", e);
            }
            let res = self.retry_failed_seq_numbers().await;
            if let Err(e) = res {
                debug!("Retry of failed seq numbers error: {:?}", e);
            }
        }
    }

    /// Store the `seq_number` in DB
    async fn store_seq_number(&self, seq_number: u64) -> Result<(), Box<dyn Error>> {
        let db = self.db.clone();
        db.connection
            .put(COUNTER_KEY, bincode::serialize(&seq_number)?)?;
        Ok(())
    }

    /// Retrieve the `seq_number` from DB
    async fn retrieve_seq_number(&self) -> Result<u64, Box<dyn Error>> {
        let db = self.db.clone();
        let seq_number = db
            .connection
            .get(COUNTER_KEY)?
            .and_then(|v| bincode::deserialize(&v).ok())
            .unwrap_or(0);
        Ok(seq_number)
    }

    /// Store the `failed_seq_numbers` in DB
    async fn store_failed_seq_numbers(&self, seq_numbers: Vec<u64>) -> Result<(), Box<dyn Error>> {
        let db = self.db.clone();
        db.connection
            .put(FAILED_SEQ_NUMBERS_KEY, bincode::serialize(&seq_numbers)?)?;
        Ok(())
    }

    /// Retrieve the `failed_seq_numbers` from DB
    async fn retrieve_failed_seq_numbers(&self) -> Result<Vec<u64>, Box<dyn Error>> {
        let db = self.db.clone();
        let seq_numbers = db
            .connection
            .get(FAILED_SEQ_NUMBERS_KEY)?
            .and_then(|v| bincode::deserialize(&v).ok())
            .unwrap_or(Vec::new());
        Ok(seq_numbers)
    }

    /// Append the `seq_number` to the `failed_seq_numbers` vec
    async fn append_failed_seq_number(&self, seq_number: u64) -> Result<(), Box<dyn Error>> {
        let mut seq_numbers = self.retrieve_failed_seq_numbers().await?;
        seq_numbers.push(seq_number);
        self.store_failed_seq_numbers(seq_numbers).await?;
        Ok(())
    }

    /// Remove the `seq_number` from the `failed_seq_numbers` vec
    async fn remove_failed_seq_number(&self, seq_number: u64) -> Result<(), Box<dyn Error>> {
        let mut seq_numbers = self.retrieve_failed_seq_numbers().await?;
        seq_numbers.retain(|&x| x != seq_number);
        self.store_failed_seq_numbers(seq_numbers).await?;
        Ok(())
    }

    /// Submit the OpenRank compute result TXs
    async fn submit_single_compute_result_txs(
        &self,
        seq_number: u64,
    ) -> Result<SubmitSingleComputeResultResult, Box<dyn Error>> {
        // fetch compute result with `seq_number`
        let compute_result = self.fetch_openrank_compute_result(seq_number).await?;

        if compute_result.compute_verification_tx_hashes().is_empty() {
            info!("Compute Job not yet verified, skipping submittion...");
            return Ok(SubmitSingleComputeResultResult::NotVerified(seq_number));
        };

        // prepare args for fetching txs
        let mut txs_args = vec![(
            "compute_commitment",
            compute_result.compute_commitment_tx_hash().clone(),
        )];
        for tx_hash in compute_result.compute_verification_tx_hashes() {
            txs_args.push(("compute_verification", tx_hash.clone()));
        }

        // fetch & submit txs, with args
        for (tx_type, tx_hash) in txs_args {
            let tx = self.fetch_openrank_tx(tx_type.to_string(), tx_hash).await?;
            self.submit_openrank_tx(tx).await?;
        }

        Ok(SubmitSingleComputeResultResult::Submitted)
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

    #[tokio::test]
    async fn test_submit_openrank_tx() -> Result<()> {
        // Spin up a local Anvil node.
        // Ensure `anvil` is available in $PATH.
        let anvil = Anvil::new().try_spawn()?;

        // Set up signer from the first default Anvil account (Alice).
        let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
        let wallet = EthereumWallet::from(signer.clone());

        // Create a provider with the wallet.
        let chain_rpc_url: Url = anvil.endpoint().parse()?;
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(chain_rpc_url.clone());

        // Deploy the `ComputeManager` contract.
        let submitters = vec![signer.address()];
        let computers = vec![address!("13978aee95f38490e9769c39b2773ed763d9cd5f")];
        let verifiers = vec![address!("cd2a3d9f938e13cd947ec05abc7fe734df8dd826")];
        let contract = ComputeManager::deploy(&provider, submitters, computers, verifiers).await?;

        // Create a contract instance.
        let contract_address = *contract.address();
        let mock_db_path = "mock-db";
        let connection = DB::open_default(mock_db_path)?;
        let mock_db = Db {
            connection,
            db_path: mock_db_path.to_string(),
        };
        let client = ComputeManagerClient::new(
            contract_address,
            chain_rpc_url,
            signer,
            "mock_openrank_rpc".to_string(),
            mock_db,
            0, // mock interval
        );

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
