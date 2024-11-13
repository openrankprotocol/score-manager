use openrank_smart_contract_client as client;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let smc = client::ComputeManagerClient::init()?;
    let res = smc.run().await;
    if let Err(e) = res {
        eprintln!("{}", e);
    }
    Ok(())
}
