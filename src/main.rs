use anyhow::{Error, Result};
use mc_scanner::slp;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let response = slp!("209.222.115.50:25565".parse()?, "mc.hypixel.net").await?;
    let config = bincode::config::standard();
    let mut file = std::fs::File::create("/home/caek/mc-scanner/bincode")?;
    bincode::encode_into_std_write(response, &mut file, config)?;
    return Ok(());
}
