use essential_node_db as node_db;
use rusqlite::Connection;

#[test]
fn create_tables() {
    let mut conn = Connection::open_in_memory().unwrap();
    let tx = conn.transaction().unwrap();
    node_db::create_tables(&tx).unwrap();
    tx.commit().unwrap();

    // Verify that each table exists by querying the SQLite master table
    for table in node_db::sql::table::ALL {
        let query = format!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='{}';",
            table.name,
        );
        let result: String = conn
            .query_row(&query, (), |row| row.get(0))
            .expect(&format!("Table {} does not exist", table.name));
        assert_eq!(
            result, table.name,
            "Table {} was not created successfully",
            table.name,
        );
    }
}