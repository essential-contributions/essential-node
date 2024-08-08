use essential_node::{self as node, Node};

fn test_conf(id: &str) -> node::Config {
    let mut conf = node::Config::default();
    conf.db.source = node::db::Source::Memory(id.to_string());
    conf
}

#[test]
fn test_node_new() {
    let conf = test_conf("test_node_new");
    Node::new(&conf).unwrap();
}

#[tokio::test]
async fn test_node_close() {
    let conf = test_conf("test_node_close");
    let node = Node::new(&conf).unwrap();
    node.close().unwrap();
}
