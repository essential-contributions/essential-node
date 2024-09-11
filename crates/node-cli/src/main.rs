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
    /// The address to bind to for the TCP listener that will be used to serve the API.
    #[arg(long, default_value_t = SocketAddrV4::new([0; 4].into(), 0).into())]
    bind_address: SocketAddr,
    /// The URL address of the Essential server that will act as the layer-1.
    ///
    /// Note: This will likely be replaced with an L1 RPC URL flag upon switching to
    /// use of Ethereum (or Ethereum test-net) as an L1.
    #[arg(long)]
    server_address: String,
    /// The type of DB storage to use.
    ///
    /// In the case that "persistent" is specified, assumes the default path.
    #[arg(long, default_value_t = Db::Memory, value_enum)]
    db: Db,
    /// The path to the node's sqlite database.
    ///
    /// Specifying this overrides the `db` type as `persistent`.
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
    /// The maximum number of TCP streams to be served simultaneously.
    #[arg(long, default_value_t = node_api::DEFAULT_CONNECTION_LIMIT)]
    tcp_conn_limit: usize,
    /// Disable the API.
    #[arg(long, default_value_t = false)]
    disable_api: bool,
    /// Disable the relayer.
    #[arg(long, default_value_t = false)]
    disable_relayer: bool,
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

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();
    if let Err(_err) = run(args).await {
        #[cfg(feature = "tracing")]
        tracing::error!("{_err}");
    }
}

/// Run the essential node.
async fn run(args: Args) -> anyhow::Result<()> {
    // If both API and relayer are disabled, exit.
    if args.disable_api && args.disable_relayer {
        anyhow::bail!("both API and relayer are disabled");
    }

    // Initialise tracing.
    if !args.disable_tracing {
        #[cfg(feature = "tracing")]
        init_tracing_subscriber()
    }

    // Start the node.
    let conf = conf_from_args(&args)?;
    #[cfg(feature = "tracing")]
    {
        tracing::debug!("Node config:\n{:#?}", conf);
        tracing::info!("Starting node");
    }
    let node = Node::new(&conf)?;

    // Run the relayer and state derivation.
    let node_handle = if !args.disable_relayer {
        #[cfg(feature = "tracing")]
        tracing::info!(
            "Starting relayer and state derivation (relaying from {:?})",
            args.server_address
        );
        Some(node.run(args.server_address)?)
    } else {
        None
    };

    let new_block = node_handle.as_ref().map_or(None, |h| Some(h.new_block()));
    let conn_pool = node.db();
    let api = async move {
        if !args.disable_api {
            let api_state = node_api::State {
                new_block,
                conn_pool,
            };
            let router = node_api::router(api_state);
            let listener = tokio::net::TcpListener::bind(args.bind_address).await?;
            #[cfg(feature = "tracing")]
            tracing::info!("Starting API server at {}", listener.local_addr()?);
            anyhow::Result::<()>::Ok(node_api::serve(&router, &listener, args.tcp_conn_limit).await)
        } else {
            anyhow::Result::<()>::Ok(futures::future::pending::<()>().await)
        }
    };

    let node_handle = async move {
        if let Some(node_handle) = node_handle {
            node_handle.join().await
        } else {
            Ok(futures::future::pending::<()>().await)
        }
    };

    // Select the first future to complete to close.
    // TODO: We should select over relayer/state-derivation critical error here.
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::select! {
        _ = api => {},
        _ = ctrl_c => {},
        r = node_handle => {
            if let Err(e) = r {
                #[cfg(feature = "tracing")]
                tracing::error!("Critical error on relayer or state derivation streams: {e}")
            }
        },
    }

    node.close().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
