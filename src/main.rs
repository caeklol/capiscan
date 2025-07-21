use anyhow::{Error, Result};
use mc_scanner::slp::server_list_ping;

fn main() -> Result<(), Error> {
    println!("{:#?}", server_list_ping("209.222.115.50:25565".parse()?)?);
    return Ok(());
}
