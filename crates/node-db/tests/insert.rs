use essential_hash::content_addr;
use essential_node_db as node_db;
use rusqlite::Connection;
use std::time::Duration;

mod util;

#[test]
fn test_insert_block() {
    // Create an in-memory SQLite database
    let conn = Connection::open_in_memory().unwrap();

    // Create the necessary tables
    node_db::create_tables(&conn).unwrap();

    let block = util::test_block(1, Duration::from_secs(1));

    // Insert the block into the database
    node_db::insert_block(&conn, &block).unwrap();

    // Verify that the block was inserted correctly
    let query = "SELECT number, created_at_seconds, created_at_nanos FROM block WHERE number = 1";
    let mut stmt = conn.prepare(query).unwrap();
    let mut rows = stmt.query(()).unwrap();

    let row = rows.next().unwrap().unwrap();
    let id: i64 = row.get(0).expect("number");
    let created_at_seconds: i64 = row.get(1).expect("created_at_seconds");
    let created_at_nanos: i32 = row.get(2).expect("created_at_nanos");

    assert_eq!(id, block.number as i64);
    assert_eq!(created_at_seconds, block.timestamp.as_secs() as i64);
    assert_eq!(created_at_nanos, block.timestamp.subsec_nanos() as i32);

    // Verify that the solutions were inserted correctly
    for solution in &block.solutions {
        let ca_blob = node_db::encode(&content_addr(solution));
        let solution_blob = node_db::encode(solution);
        let query = "SELECT solution FROM solution WHERE content_hash = ?";
        let mut stmt = conn.prepare(query).unwrap();
        let mut rows = stmt.query(&[&ca_blob]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let solution_data: String = row.get(0).unwrap();
        assert_eq!(solution_data, solution_blob);
    }
}
