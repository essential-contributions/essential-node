use core::panic;

use tokio::sync::oneshot;

use crate::error::CriticalError;

use super::*;

#[tokio::test]
async fn test_close() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    // Close when close is called
    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
        }
    });

    let close = Close {
        close_contracts,
        close_blocks,
    };

    assert!(!c.is_finished());
    assert!(!b.is_finished());

    close.close();
    c.await.unwrap();
    b.await.unwrap();

    // Continues to close after close is called once
    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
        }
    });
    c.await.unwrap();
    b.await.unwrap();

    // Close on drop
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());
    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
        }
    });

    let close = Close {
        close_contracts,
        close_blocks,
    };

    drop(close);

    c.await.unwrap();
    b.await.unwrap();
}

#[tokio::test]
async fn test_handle_close_ok() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            Ok(())
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    handle.close().await.unwrap();
}

#[tokio::test]
async fn test_handle_close_immediate_ok() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let contracts = contracts.clone();
        async move {
            // Avoid drop but don't await
            let _c = contracts;
            Ok(())
        }
    });
    let b = tokio::spawn({
        let blocks = blocks.clone();
        async move {
            // Avoid drop but don't await
            let _b = blocks;
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    handle.close().await.unwrap();
}

#[tokio::test]
async fn test_drop_handle() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());
    let (c_close, mut c_closed) = oneshot::channel();
    let (b_close, mut b_closed) = oneshot::channel();

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            let _ = c_close.send(());
            Ok(())
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            let _ = b_close.send(());
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    assert_eq!(
        c_closed.try_recv().unwrap_err(),
        oneshot::error::TryRecvError::Empty
    );
    assert_eq!(
        b_closed.try_recv().unwrap_err(),
        oneshot::error::TryRecvError::Empty
    );
    drop(handle);
    c_closed.await.unwrap();
    b_closed.await.unwrap();
}

#[tokio::test]
async fn test_drop_handle_join() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());
    let (c_close, mut c_closed) = oneshot::channel();
    let (b_close, mut b_closed) = oneshot::channel();

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            let _ = c_close.send(());
            Ok(())
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            let _ = b_close.send(());
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    assert_eq!(
        c_closed.try_recv().unwrap_err(),
        oneshot::error::TryRecvError::Empty
    );
    assert_eq!(
        b_closed.try_recv().unwrap_err(),
        oneshot::error::TryRecvError::Empty
    );
    let f = handle.join();

    drop(f);
    c_closed.await.unwrap();
    b_closed.await.unwrap();
}

#[tokio::test]
async fn test_select_handle_join() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());
    let (c_close, mut c_closed) = oneshot::channel();
    let (b_close, mut b_closed) = oneshot::channel();

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            let _ = c_close.send(());
            Ok(())
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            let _ = b_close.send(());
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    assert_eq!(
        c_closed.try_recv().unwrap_err(),
        oneshot::error::TryRecvError::Empty
    );
    assert_eq!(
        b_closed.try_recv().unwrap_err(),
        oneshot::error::TryRecvError::Empty
    );
    let f = handle.join();

    tokio::pin!(f);

    tokio::select! {
        _ = &mut f => {}
        _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
    }
    assert_eq!(
        c_closed.try_recv().unwrap_err(),
        oneshot::error::TryRecvError::Empty
    );
    assert_eq!(
        b_closed.try_recv().unwrap_err(),
        oneshot::error::TryRecvError::Empty
    );
}

#[tokio::test]
async fn test_drop_handle_close() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());
    let (c_close, mut c_closed) = oneshot::channel();
    let (b_close, mut b_closed) = oneshot::channel();

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            let _ = c_close.send(());
            Ok(())
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            let _ = b_close.send(());
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    assert_eq!(
        c_closed.try_recv().unwrap_err(),
        oneshot::error::TryRecvError::Empty
    );
    assert_eq!(
        b_closed.try_recv().unwrap_err(),
        oneshot::error::TryRecvError::Empty
    );
    let f = handle.close();
    drop(f);
    c_closed.await.unwrap();
    b_closed.await.unwrap();
}

#[tokio::test]
#[should_panic]
async fn test_panic_task_close_contracts_immediate() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let contracts = contracts.clone();
        async move {
            let _c = contracts;
            panic!("contracts");
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let _ = handle.close().await;
}

#[tokio::test]
#[should_panic]
async fn test_panic_task_close_blocks() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            Ok(())
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            panic!("blocks");
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let _ = handle.close().await;
}

#[tokio::test]
#[should_panic]
async fn test_panic_task_join_contracts_immediate() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let contracts = contracts.clone();
        async move {
            let _c = contracts;
            panic!("contracts");
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let _ = handle.join().await;
}

#[tokio::test]
async fn test_handle_close_both_err() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            Err(CriticalError::Overflow)
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            Err(CriticalError::UrlParse)
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let e = handle.close().await.unwrap_err();

    // Blocks error is always returned first
    assert!(matches!(e, CriticalError::UrlParse));
}

#[tokio::test]
async fn test_handle_close_both_immediate_err() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let contracts = contracts.clone();
        async move {
            // Avoid drop but don't await
            let _c = contracts;
            Err(CriticalError::Overflow)
        }
    });
    let b = tokio::spawn({
        let blocks = blocks.clone();
        async move {
            // Avoid drop but don't await
            let _b = blocks;
            Err(CriticalError::UrlParse)
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let e = handle.close().await.unwrap_err();

    // Blocks error is always returned first
    assert!(matches!(e, CriticalError::UrlParse));
}

#[tokio::test]
async fn test_handle_close_contracts_err() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            Err(CriticalError::Overflow)
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let e = handle.close().await.unwrap_err();

    assert!(matches!(e, CriticalError::Overflow));
}

#[tokio::test]
async fn test_handle_close_blocks_err() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            Ok(())
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            Err(CriticalError::Overflow)
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let e = handle.close().await.unwrap_err();

    assert!(matches!(e, CriticalError::Overflow));
}

#[tokio::test]
async fn test_handle_close_contracts_immediate_err() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let contracts = contracts.clone();
        async move {
            // Avoid drop but don't await
            let _c = contracts;
            Err(CriticalError::Overflow)
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let e = handle.close().await.unwrap_err();

    assert!(matches!(e, CriticalError::Overflow));
}

#[tokio::test]
async fn test_handle_close_blocks_immediate_err() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            Ok(())
        }
    });
    let b = tokio::spawn({
        let blocks = blocks.clone();
        async move {
            // Avoid drop but don't await
            let _b = blocks;
            Err(CriticalError::Overflow)
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let e = handle.close().await.unwrap_err();

    assert!(matches!(e, CriticalError::Overflow));
}

#[tokio::test]
async fn test_handle_join_ok() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            Ok(())
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    tokio::time::timeout(std::time::Duration::from_millis(50), handle.join())
        .await
        .unwrap_err();
}

#[tokio::test]
async fn test_handle_join_immediate_ok() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let contracts = contracts.clone();
        async move {
            // Avoid drop but don't await
            let _c = contracts;
            Ok(())
        }
    });
    let b = tokio::spawn({
        let blocks = blocks.clone();
        async move {
            // Avoid drop but don't await
            let _b = blocks;
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    handle.join().await.unwrap();
}

#[tokio::test]
async fn test_handle_join_both_immediate_err() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let contracts = contracts.clone();
        async move {
            // Avoid drop but don't await
            let _c = contracts;
            Err(CriticalError::Overflow)
        }
    });
    let b = tokio::spawn({
        let blocks = blocks.clone();
        async move {
            // Avoid drop but don't await
            let _b = blocks;
            Err(CriticalError::UrlParse)
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let e = handle.join().await.unwrap_err();

    // Because of select either task's result can be returned
    assert!(matches!(e, CriticalError::UrlParse) || matches!(e, CriticalError::Overflow));
}

#[tokio::test]
async fn test_handle_join_contracts_immediate_err() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let contracts = contracts.clone();
        async move {
            // Avoid drop but don't await
            let _c = contracts;
            Err(CriticalError::Overflow)
        }
    });
    let b = tokio::spawn({
        let mut blocks = blocks.clone();
        async move {
            let _ = blocks.changed().await;
            Ok(())
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let e = handle.join().await.unwrap_err();

    assert!(matches!(e, CriticalError::Overflow));
}

#[tokio::test]
async fn test_handle_join_blocks_immediate_err() {
    let (close_contracts, contracts) = watch::channel(());
    let (close_blocks, blocks) = watch::channel(());

    let c = tokio::spawn({
        let mut contracts = contracts.clone();
        async move {
            let _ = contracts.changed().await;
            Ok(())
        }
    });
    let b = tokio::spawn({
        let blocks = blocks.clone();
        async move {
            // Avoid drop but don't await
            let _b = blocks;
            Err(CriticalError::Overflow)
        }
    });

    let handle = Handle::new(c, b, close_contracts, close_blocks);
    let e = handle.join().await.unwrap_err();

    assert!(matches!(e, CriticalError::Overflow));
}
