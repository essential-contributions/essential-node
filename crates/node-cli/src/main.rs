use clap::{Parser, ValueEnum};
use essential_node::{self as node, db::Config, RunConfig};
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
    /// The endpoint of the node that will act as the layer-1.
    ///
    /// If this is `None`, then the relayer stream will not run.
    ///
    /// Note: This will likely be replaced with an L1 RPC URL flag upon switching to
    /// use of Ethereum (or Ethereum test-net) as an L1.
    #[arg(long)]
    relayer_source_endpoint: Option<String>,
    /// Disable the state derivation stream.
    #[arg(long)]
    disable_state_derivation: bool,
    /// Disable the validation stream.
    #[arg(long)]
    disable_validation: bool,
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
    #[arg(long, default_value_t = Config::default_conn_limit())]
    db_conn_limit: usize,
    /// Disable the tracing subscriber.
    #[arg(long)]
    disable_tracing: bool,
    /// The maximum number of TCP streams to be served simultaneously.
    #[arg(long, default_value_t = node_api::DEFAULT_CONNECTION_LIMIT)]
    tcp_conn_limit: usize,
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
fn conf_from_args(args: &Args) -> anyhow::Result<Config> {
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
    let config = Config::new(source, conn_limit);
    Ok(config)
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
    let db = node::db(&conf)?;

    // Run the node with specified config.
    let Args {
        relayer_source_endpoint,
        disable_state_derivation,
        disable_validation,
        ..
    } = args;

    #[cfg(feature = "tracing")]
    tracing::info!(
        "Starting {}{}{}",
        if disable_state_derivation {
            ""
        } else {
            "state derivation"
        },
        if disable_validation {
            "".to_string()
        } else {
            format!(
                "{}{}",
                if disable_state_derivation {
                    ""
                } else if relayer_source_endpoint.is_some() {
                    ", "
                } else {
                    " and "
                },
                "validation"
            )
        },
        if let Some(node_endpoint) = relayer_source_endpoint.as_ref() {
            format!(
                "{}relayer (relaying from {:?})",
                if disable_state_derivation && disable_validation {
                    ""
                } else {
                    " and "
                },
                node_endpoint,
            )
        } else {
            "".to_string()
        }
    );

    let block_tx = node::BlockTx::new();
    let block_rx = block_tx.new_listener();

    let run_conf = RunConfig {
        relayer_source_endpoint: relayer_source_endpoint.clone(),
        run_state_derivation: !disable_state_derivation,
        run_validation: !disable_validation,
    };
    let node_handle = node::run(db.clone(), run_conf, block_tx)?;
    let node_future = async move {
        if relayer_source_endpoint.is_none() && disable_state_derivation && disable_validation {
            node_handle.join().await
        } else {
            std::future::pending().await
        }
    };

    // Run the API.
    let api_state = node_api::State {
        new_block: Some(block_rx),
        conn_pool: db.clone(),
    };
    let router = node_api::router(api_state);
    let listener = tokio::net::TcpListener::bind(args.bind_address).await?;
    #[cfg(feature = "tracing")]
    tracing::info!("Starting API server at {}", listener.local_addr()?);
    let api = node_api::serve(&router, &listener, args.tcp_conn_limit);

    // Select the first future to complete to close.
    // TODO: We should select over relayer/state-derivation critical error here.
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::select! {
        _ = api => {},
        _ = ctrl_c => {},
        r = node_future => {
            if let Err(e) = r {
                #[cfg(feature = "tracing")]
                tracing::error!("Critical error on relayer, state derivation or validation streams: {e}")
            }
        },
    }

    db.close().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
