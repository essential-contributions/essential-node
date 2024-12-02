use anyhow::Context;
use clap::{Parser, ValueEnum};
use essential_node::{self as node, RunConfig};
use essential_node_api as node_api;
use essential_node_types::{block_notify::BlockTx, BigBang};
use std::{
    net::{SocketAddr, SocketAddrV4},
    path::{Path, PathBuf},
};

#[cfg(test)]
mod tests;

/// The Essential Node CLI.
#[derive(Parser, Clone)]
#[command(version, about)]
pub struct Args {
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
    #[arg(long, default_value_t = node::db::pool::Config::default_conn_limit())]
    api_db_conn_limit: usize,
    /// The number of simultaneous sqlite DB connections to maintain for the node's relayer and
    /// validation streams.
    ///
    /// This is unique from the API connection limit in order to ensure that the node's relayer and
    /// validation streams have high-priority DB connection access.
    ///
    /// By default, this is the number of available CPUs multiplied by 4.
    #[arg(long, default_value_t = node::db::pool::Config::default_conn_limit())]
    node_db_conn_limit: usize,
    /// Disable the tracing subscriber.
    #[arg(long)]
    disable_tracing: bool,
    /// The maximum number of TCP streams to be served simultaneously.
    #[arg(long, default_value_t = node_api::DEFAULT_CONNECTION_LIMIT)]
    tcp_conn_limit: usize,
    /// Specify a path to the `big-bang.yml` configuration.
    ///
    /// This specifies the genesis configuration, which includes items like the contract registry
    /// address, block state address and associated big-bang state.
    ///
    /// If no configuration is specified, defaults to the `BigBang::default()` implementation.
    ///
    /// To learn more, see the API docs for the `essential_node_types::BigBang` type.
    #[arg(long)]
    big_bang: Option<std::path::PathBuf>,
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

/// Construct the node's DB config from the parsed args.
fn node_db_conf_from_args(args: &Args) -> anyhow::Result<node::db::pool::Config> {
    let source = match (&args.db, &args.db_path) {
        (Db::Memory, None) => node::db::pool::Source::default_memory(),
        (_, Some(path)) => node::db::pool::Source::Path(path.clone()),
        (Db::Persistent, None) => {
            let Some(path) = default_db_path() else {
                anyhow::bail!("unable to detect user's data directory for default DB path")
            };
            node::db::pool::Source::Path(path)
        }
    };
    let conn_limit = args.node_db_conn_limit;
    let config = node::db::pool::Config::new(source, conn_limit);
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

/// Load the big bang configuration from the yml file at the given path, or produce the default if
/// no path is given.
fn load_big_bang_or_default(path: Option<&Path>) -> anyhow::Result<BigBang> {
    match path {
        None => Ok(BigBang::default()),
        Some(path) => {
            let big_bang_str = std::fs::read_to_string(path)
                .context("failed to read big bang configuration from path")?;
            serde_yaml::from_str(&big_bang_str)
                .context("failed to deserialize big bang configuration from YAML string")
        }
    }
}
/// Run the essential node.
pub async fn run(args: Args) -> anyhow::Result<()> {
    // Initialise tracing.
    if !args.disable_tracing {
        #[cfg(feature = "tracing")]
        init_tracing_subscriber()
    }

    // Start the node.
    let node_db_conf = node_db_conf_from_args(&args)?;
    #[cfg(feature = "tracing")]
    {
        tracing::debug!("Node DB config:\n{:#?}", node_db_conf);
        tracing::info!("Starting node");
    }
    let node_db = node::db::ConnectionPool::with_tables(&node_db_conf)?;

    // Load the big bang configuration, and ensure the big bang block exists.
    let big_bang = load_big_bang_or_default(args.big_bang.as_deref())?;
    node::ensure_big_bang_block(&node_db, &big_bang)
        .await
        .context("failed to ensure big bang block")?;

    // Run the node with specified config.
    let Args {
        relayer_source_endpoint,
        disable_validation,
        ..
    } = args;

    #[cfg(feature = "tracing")]
    tracing::info!(
        "Starting {}{}",
        if disable_validation {
            "".to_string()
        } else {
            "validation".to_string()
        },
        if let Some(node_endpoint) = relayer_source_endpoint.as_ref() {
            format!(
                "{}relayer (relaying from {:?})",
                if disable_validation { "" } else { " and " },
                node_endpoint,
            )
        } else {
            "".to_string()
        }
    );

    let block_tx = BlockTx::new();
    let block_rx = block_tx.new_listener();

    let run_conf = RunConfig {
        relayer_source_endpoint: relayer_source_endpoint.clone(),
        run_validation: !disable_validation,
    };
    let node_handle = node::run(
        node_db.clone(),
        run_conf,
        big_bang.contract_registry.contract,
        big_bang.program_registry.contract,
        block_tx,
    )?;
    let node_future = async move {
        if relayer_source_endpoint.is_none() && disable_validation {
            std::future::pending().await
        } else {
            let r = node_handle.join().await;
            if r.is_ok() && relayer_source_endpoint.is_none() {
                #[cfg(feature = "tracing")]
                tracing::info!("Node has completed all streams and is now idle");
                std::future::pending().await
            }
            r
        }
    };

    // Run the API with its own connection pool.
    let api_db_conf = node::db::pool::Config {
        conn_limit: args.api_db_conn_limit,
        ..node_db_conf
    };
    #[cfg(feature = "tracing")]
    tracing::debug!("API DB config:\n{:#?}", api_db_conf);
    let api_db = node::db::ConnectionPool::with_tables(&api_db_conf)?;
    let api_state = node_api::State {
        new_block: Some(block_rx),
        conn_pool: api_db.clone(),
    };
    let router = node_api::router(api_state);
    let listener = tokio::net::TcpListener::bind(args.bind_address).await?;
    #[cfg(feature = "tracing")]
    tracing::info!("Starting API server at {}", listener.local_addr()?);
    let api = node_api::serve(&router, &listener, args.tcp_conn_limit);

    // Select the first future to complete to close.
    // TODO: We should select over relayer / validation critical error here.
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::select! {
        _ = api => {},
        _ = ctrl_c => {},
        r = node_future => {
            if let Err(e) = r {
                #[cfg(feature = "tracing")]
                tracing::error!("Critical error on relayer or validation stream: {e}")
            }
        },
    }

    node_db.close().map_err(|e| anyhow::anyhow!("{e}"))?;
    api_db.close().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
