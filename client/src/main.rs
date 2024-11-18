use clap::{Parser, Subcommand};
use openrank_common::tx::TxHash;
use openrank_smart_contract_client as client;
use std::error::Error;

#[derive(Debug, Clone, Subcommand)]
/// The method to call.
enum Method {
    /// Post Openrank TX into on-chain smart contract
    PostTxOnChain { tx_id: String },

    /// Post Openrank TXs into on-chain smart contract, in periodic interval
    StartIntervalSubmit,
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

/// 1. Creates a new `Client`.
/// 2. Start interval submission process.
async fn start_interval_submit() -> Result<(), Box<dyn Error>> {
    // Creates a new client
    let smc = client::ComputeManagerClient::init()?;

    smc.start_interval_submit().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Set up logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let cli = Args::parse();

    match cli.method {
        Method::PostTxOnChain { tx_id } => {
            post_tx_on_chain(tx_id).await?;
        }
        Method::StartIntervalSubmit => {
            start_interval_submit().await?;
        }
    }
    Ok(())
}
