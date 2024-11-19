use clap::{Parser, Subcommand};
use openrank_common::tx::TxHash;
use openrank_smart_contract_client as client;
use std::error::Error;

#[derive(Debug, Clone, Subcommand)]
/// The method to call.
enum Method {
    /// Post Openrank TX into on-chain smart contract
    PostTxOnChain { tx_id: String },
    /// Get signer
    GetSigner { tx_id: String },
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// The command line arguments.
struct Args {
    #[command(subcommand)]
    method: Method,
}

/// 1. Creates a new `Client`.
/// 2. Calls the Sequencer to get the TX given a TX hash.
/// 3. Submits the TX into the on-chain smart contract.
async fn post_tx_on_chain(arg: String) -> Result<(), Box<dyn Error>> {
    // Creates a new client
    let smc = client::ComputeManagerClient::init()?;

    // Parse the arg
    let (prefix, tx_hash) = arg.split_once(':').ok_or("Failed to parse argument")?;
    let tx_hash_bytes = hex::decode(tx_hash)?;
    let tx_hash = TxHash::from_bytes(tx_hash_bytes);

    // Fetch the tx
    let tx = smc.fetch_openrank_tx(prefix.to_string(), tx_hash).await?;

    // Submit the tx
    smc.submit_openrank_tx(tx).await?;

    Ok(())
}

async fn get_signer(arg: String) -> Result<(), Box<dyn Error>> {
    // Creates a new client
    let smc = client::ComputeManagerClient::init()?;

    // Parse the arg
    let (prefix, tx_hash) = arg.split_once(':').ok_or("Failed to parse argument")?;
    let tx_hash_bytes = hex::decode(tx_hash)?;
    let tx_hash = TxHash::from_bytes(tx_hash_bytes);

    // Fetch the tx
    let tx = smc.fetch_openrank_tx(prefix.to_string(), tx_hash).await?;
    let signer = smc.get_signer(tx.clone()).await?;

    let address = tx.verify().unwrap();
    println!("address: {}", address);
    println!("signer: {}", signer);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Args::parse();

    match cli.method {
        Method::PostTxOnChain { tx_id } => {
            post_tx_on_chain(tx_id).await?;
        }
        Method::GetSigner { tx_id } => {
            get_signer(tx_id).await?;
        }
    }
    Ok(())
}
