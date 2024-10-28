use clap::Parser;
use essential_node_cli::Args;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();
    if let Err(_err) = essential_node_cli::run(args).await {
        #[cfg(feature = "tracing")]
        tracing::error!("{_err:?}");
    }
}
