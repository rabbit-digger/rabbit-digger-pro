mod config;
mod plugins;

use anyhow::Result;
use plugins::load_plugins;

#[tokio::main]
async fn main() -> Result<()> {
    let plugins = load_plugins()?;
    println!("plugins: {:?}", plugins);

    Ok(())
}
