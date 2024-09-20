use super::*;
use error::RecoverableError;
use futures::FutureExt;
use std::sync::{atomic::AtomicUsize, Arc};

#[tokio::test]
async fn test_run_critical_err() {
    let handle =
        run(|_s| futures::future::ready::<InternalResult<()>>(Err(CriticalError::Overflow.into())))
            .unwrap();
    let e = handle.join().await.unwrap_err();
    assert!(matches!(e, CriticalError::Overflow));
}

#[tokio::test]
async fn test_run_recoverable_err() {
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let handle = run(move |_shutdown| {
        if c.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            futures::future::ready::<InternalResult<()>>(Err(RecoverableError::NonSequentialBlock(
                0, 2,
            )
            .into()))
        } else {
            futures::future::ready::<InternalResult<()>>(Ok(()))
        }
    })
    .unwrap();
    handle.join().await.unwrap();
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_run_recoverable_close() {
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let handle = run(move |mut shutdown| {
        if c.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            futures::future::ready::<InternalResult<()>>(Err(RecoverableError::NonSequentialBlock(
                0, 2,
            )
            .into()))
            .boxed()
        } else {
            async move {
                let _ = shutdown.changed().await;
                Ok(())
            }
            .boxed()
        }
    })
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    handle.close().await.unwrap();
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_run_join() {
    let count = Arc::new(());
    let c = count.clone();
    let handle = run(move |mut shutdown| {
        let c = c.clone();
        async move {
            let _c = c;
            let _ = shutdown.changed().await;
            Ok(())
        }
    })
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_millis(100), handle.join())
        .await
        .unwrap_err();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    // Once the join future is dropped both tasks should be end
    Arc::try_unwrap(count).unwrap();
}

#[tokio::test]
async fn test_run_close() {
    let count = Arc::new(());
    let c = count.clone();
    let handle = run(move |mut shutdown| {
        let c = c.clone();
        async move {
            let _c = c;
            let _ = shutdown.changed().await;
            Ok(())
        }
    })
    .unwrap();
    handle.close().await.unwrap();
    Arc::try_unwrap(count).unwrap();
}

#[tokio::test]
async fn test_run_join_immediate() {
    let count = Arc::new(());
    let c = count.clone();
    let handle = run(move |shutdown| {
        let c = c.clone();
        async move {
            let _c = c;
            let _s = shutdown;
            Ok(())
        }
    })
    .unwrap();
    handle.join().await.unwrap();
    Arc::try_unwrap(count).unwrap();
}

#[tokio::test]
async fn test_run_multiple_recoverable() {
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let handle = run(move |mut shutdown| {
        if c.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 10 {
            futures::future::ready::<InternalResult<()>>(Err(RecoverableError::NonSequentialBlock(
                0, 2,
            )
            .into()))
            .boxed()
        } else {
            async move {
                let _ = shutdown.changed().await;
                Ok(())
            }
            .boxed()
        }
    })
    .unwrap();
    while count.load(std::sync::atomic::Ordering::SeqCst) < 10 {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 11);

    // Still running
    let count = Arc::try_unwrap(count).unwrap_err();

    handle.close().await.unwrap();

    // Closed
    Arc::try_unwrap(count).unwrap();
}
