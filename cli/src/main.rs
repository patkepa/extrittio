use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    extrittio_cli::run().await
}
