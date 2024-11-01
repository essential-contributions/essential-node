use tokio::sync::watch::{channel, error::RecvError, Receiver, Sender};

/// Wrapper around `watch::Sender` to notify of new blocks.
///
/// This is used by `essential-builder` to notify `essential-relayer`
/// and by `essential-relayer` to notify [`validation`] stream.
#[derive(Clone, Default)]
pub struct BlockTx(Sender<()>);

/// Wrapper around `watch::Receiver` to listen to new blocks.
///
/// This is used by [`db::subscribe_blocks`] stream.
#[derive(Clone)]
pub struct BlockRx(Receiver<()>);

impl BlockTx {
    /// Create a new [`BlockTx`] to notify listeners of new blocks.
    pub fn new() -> Self {
        let (block_tx, _block_rx) = channel(());
        Self(block_tx)
    }

    /// Notify listeners that a new block has been received.
    ///
    /// Note this is best effort and will still send even if there are currently no listeners.
    pub fn notify(&self) {
        let _ = self.0.send(());
    }

    /// Create a new [`BlockRx`] to listen for new blocks.
    pub fn new_listener(&self) -> BlockRx {
        BlockRx(self.0.subscribe())
    }

    /// Get the number of receivers listening for new blocks.
    pub fn receiver_count(&self) -> usize {
        self.0.receiver_count()
    }
}

impl BlockRx {
    /// Waits for a change notification.
    pub async fn changed(&mut self) -> Result<(), RecvError> {
        self.0.changed().await
    }
}
