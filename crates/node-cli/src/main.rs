use clap::{Parser, ValueEnum};
use essential_node::{self as node, Node};
use essential_node_api as node_api;
use std::{
    net::{SocketAddr, SocketAddrV4},
    path::PathBuf,
};

/// The Essential Node CLI.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// The address to bind to for the TCP listener.
    #[arg(long, default_value_t = SocketAddrV4::new([0; 4].into(), 0).into())]
    bind_addr: SocketAddr,
    /// The type of DB storage to use.
    ///
    /// In the case that "persistent" is specified, assumes the default path.
    #[arg(long, default_value_t = Db::Memory, value_enum)]
    db: Db,
    /// The path to the node's sqlite database.
    ///
    /// Specifying this overrides the the `db` type as `persistent`.
    ///
    /// By default, this path will be within the user's data directory.
    #[arg(long)]
    db_path: Option<PathBuf>,
    /// The number of simultaneous sqlite DB connections to maintain for serving the API.
    ///
    /// By default, this is the number of available CPUs multiplied by 4.
    #[arg(long, default_value_t = node::db::Config::default_conn_limit())]
    db_conn_limit: usize,
    /// Disable the tracing subscriber.
    #[arg(long, default_value_t = false)]
    disable_tracing: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Db {
    /// Temporary, in-memory storage that lasts for the duration of the process.
    Memory,
    /// Persistent storage on the local HDD or SSD.
    ///
    /// The DB path may be specified with `--db-path`.
    Persistent,
}

// TODO: Lift this into the node lib?
fn default_db_path() -> Option<PathBuf> {
    dirs::data_dir().map(|mut path| {
        path.extend(["essential", "node", "db.sqlite"]);
        path
    })
}

/// Construct the node's config from the parsed args.
fn conf_from_args(args: &Args) -> anyhow::Result<node::Config> {
    let source = match (&args.db, &args.db_path) {
        (Db::Memory, None) => node::db::Source::default_memory(),
        (_, Some(path)) => node::db::Source::Path(path.clone()),
        (Db::Persistent, None) => {
            let Some(path) = default_db_path() else {
                anyhow::bail!("unable to detect user's data directory for default DB path")
            };
            node::db::Source::Path(path)
        }
    };
    let conn_limit = args.db_conn_limit;
    let db = node::db::Config { conn_limit, source };
    Ok(node::Config { db })
}

#[cfg(feature = "tracing")]
fn init_tracing_subscriber() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .try_init();
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(err) = run(args).await {
        tracing::error!("{err}");
    }
}

/// Run the essential node.
async fn run(args: Args) -> anyhow::Result<()> {
    // Initialise tracing.
    if !args.disable_tracing {
        #[cfg(feature = "tracing")]
        init_tracing_subscriber()
    }

    // Start the node.
    let conf = conf_from_args(&args)?;
    tracing::info!("Starting node with config:\n{:#?}", conf);
    let node = Node::new(&conf)?;

    // Run the API.
    let listener = tokio::net::TcpListener::bind(args.bind_addr).await?;
    tracing::info!("Starting API server at {}", listener.local_addr()?);
    let api = run_api(node.db(), &listener);

    // Select the first future to complete to close.
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::select! {
        _ = api => {},
        _ = ctrl_c => {},
    }
    node.close().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

/// Serve the node's API.
async fn run_api(
    node_db: node::db::ConnectionPool,
    listener: &tokio::net::TcpListener,
) -> anyhow::Result<()> {
    let router = node_api::router(node_db);
    let mut conn_set = tokio::task::JoinSet::new();
    loop {
        node_api::serve_next_conn(&router, &listener, &mut conn_set).await;
    }
}
