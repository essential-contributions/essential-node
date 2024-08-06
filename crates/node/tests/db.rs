use essential_node::{self as node, Node};

fn test_conf(id: &str) -> node::Config {
    let mut conf = node::Config::default();
    conf.db.source = node::db::Source::Memory(id.to_string());
    conf
}

#[tokio::test]
async fn test_db_conn_acquire() {
    let conf = test_conf("test_db_conn_acquire");
    let node = Node::new(&conf).unwrap();
    node.conn_pool().acquire().await.unwrap();
}

#[test]
fn test_try_db_conn_acquire() {
    let conf = test_conf("test_try_db_conn_acquire");
    let node = Node::new(&conf).unwrap();
    node.conn_pool().try_acquire().unwrap();
}
