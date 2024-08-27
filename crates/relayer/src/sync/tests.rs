use super::*;
use crate::error::InternalError;
use essential_types::predicate::{Directive, Predicate};

fn new_conn_pool() -> AsyncConnectionPool {
    AsyncConnectionPool::new(3, || {
        rusqlite::Connection::open_with_flags_and_vfs(
            "file:/test_sync_contracts",
            Default::default(),
            "memdb",
        )
    })
    .unwrap()
}

#[tokio::test]
async fn test_sync_contracts() {
    let conn = new_conn_pool();
    let mut test_conn = conn.acquire().await.unwrap();

    let tx = test_conn.transaction().unwrap();
    essential_node_db::create_tables(&tx).unwrap();
    tx.commit().unwrap();

    let predicate = Predicate {
        state_read: vec![],
        constraints: vec![],
        directive: Directive::Satisfy,
    };

    let stream = futures::stream::iter(vec![
        Ok(Contract {
            predicates: vec![predicate.clone()],
            salt: [0; 32],
        }),
        Ok(Contract {
            predicates: vec![predicate.clone()],
            salt: [1; 32],
        }),
    ]);

    sync_contracts(conn.clone(), &None, watch::channel(()).0, stream)
        .await
        .unwrap();

    let result = essential_node_db::list_contracts(&test_conn, 0..3).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, 0);
    assert_eq!(result[1].0, 1);

    assert_eq!(result[0].1.len(), 1);
    assert_eq!(result[1].1.len(), 1);
    assert_eq!(result[0].1[0].salt, [0; 32]);
    assert_eq!(result[1].1[0].salt, [1; 32]);

    let result: ContractProgress = essential_node_db::get_contract_progress(&test_conn)
        .unwrap()
        .unwrap()
        .into();

    assert_eq!(
        result,
        ContractProgress {
            l2_block_number: 1,
            last_contract: essential_hash::content_addr(&Contract {
                predicates: vec![predicate.clone()],
                salt: [1; 32],
            })
        }
    );

    let stream = futures::stream::iter(vec![
        Ok(Contract {
            predicates: vec![predicate.clone()],
            salt: [4; 32],
        }),
        Ok(Contract {
            predicates: vec![predicate.clone()],
            salt: [5; 32],
        }),
    ]);

    let progress = Some(ContractProgress {
        l2_block_number: 1,
        last_contract: essential_hash::content_addr(&Contract {
            predicates: vec![predicate.clone()],
            salt: [1; 32],
        }),
    });

    let e = sync_contracts(conn.clone(), &progress, watch::channel(()).0, stream)
        .await
        .unwrap_err();

    assert!(matches!(
        e,
        InternalError::Critical(CriticalError::DataSyncFailed(
            DataSyncError::ContractMismatch(1, _, _)
        ))
    ));

    let stream = futures::stream::iter(vec![
        Ok(Contract {
            predicates: vec![predicate.clone()],
            salt: [1; 32],
        }),
        Ok(Contract {
            predicates: vec![predicate.clone()],
            salt: [3; 32],
        }),
        Ok(Contract {
            predicates: vec![predicate.clone()],
            salt: [4; 32],
        }),
    ]);

    let progress = Some(ContractProgress {
        l2_block_number: 1,
        last_contract: essential_hash::content_addr(&Contract {
            predicates: vec![predicate.clone()],
            salt: [1; 32],
        }),
    });

    sync_contracts(conn, &progress, watch::channel(()).0, stream)
        .await
        .unwrap();
}
