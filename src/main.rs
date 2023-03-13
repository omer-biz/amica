use amica::{Args, Proxy};
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    Proxy::start(args).await?;

    Ok(())
}
