use essential_types::predicate::{Directive, Predicate};
use rusqlite::OpenFlags;

use super::*;

#[tokio::test]
async fn test_sync_contracts() {
    let mut flags = OpenFlags::default();
    flags.insert(OpenFlags::SQLITE_OPEN_SHARED_CACHE);
    let conn = rusqlite::Connection::open_with_flags("file::memory:", flags).unwrap();
    let mut test_conn = rusqlite::Connection::open_with_flags("file::memory:", flags).unwrap();

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

    sync_contracts(conn, None, stream).await.unwrap();

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
            last_contract: essential_hash::contract_addr::from_contract(&Contract {
                predicates: vec![predicate.clone()],
                salt: [1; 32],
            })
        }
    );
}
