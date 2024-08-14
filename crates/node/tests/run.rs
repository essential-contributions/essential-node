#![cfg(feature = "test-utils")]

use essential_node::{
    test_utils::{setup_server, test_db_conf},
    Node,
};

#[tokio::test]
async fn test_run() {
    let conf = test_db_conf("test_acquire");
    let node = Node::new(&conf).unwrap();
    let (server_address, _child) = setup_server().await;

    node.run(server_address).unwrap();
}
