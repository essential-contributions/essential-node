use essential_node::{test_utils::test_db_conf, Node};

#[test]
fn test_node_new() {
    let conf = test_db_conf();
    Node::new(&conf).unwrap();
}

#[tokio::test]
async fn test_node_close() {
    let conf = test_db_conf();
    let node = Node::new(&conf).unwrap();
    node.close().unwrap();
}
