use alloy::{
    network::EthereumWallet,
    node_bindings::Anvil,
    primitives::address,
    providers::ProviderBuilder,
    signers::{k256::ecdsa::SigningKey, local::PrivateKeySigner},
    transports::http::reqwest::Url,
};
use eyre::Result;

use openrank_common::{
    merkle::Hash,
    tx::{
        compute::{Commitment, Request, Verification},
        Body, Tx, TxHash,
    },
};

use openrank_smart_contract_client::{ComputeManager, ComputeManagerClient, Db};

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
    let mock_db = Db::new(mock_db_path)?;
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
