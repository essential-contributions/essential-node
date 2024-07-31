use essential_relayer::Relayer;
use essential_types::{
    contract::{Contract, SignedContract},
    predicate::{Directive, Predicate},
};
use reqwest::ClientBuilder;
use rusqlite::OpenFlags;
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

#[tokio::test]
async fn test_sync_contracts() {
    let (server_address, _child) = setup_server().await;

    let client = ClientBuilder::new()
        .http2_prior_knowledge()
        .build()
        .unwrap();
    let url = reqwest::Url::parse(server_address.as_str())
        .unwrap()
        .join("/deploy-contract")
        .unwrap();

    let mut flags = OpenFlags::default();
    flags.insert(OpenFlags::SQLITE_OPEN_SHARED_CACHE);
    let mut conn = rusqlite::Connection::open_with_flags("file::memory:", flags).unwrap();
    let tx = conn.transaction().unwrap();
    essential_node_db::create_tables(&tx).unwrap();
    tx.commit().unwrap();
    let test_conn = rusqlite::Connection::open_with_flags("file::memory:", flags).unwrap();

    let predicate = Predicate {
        state_read: vec![],
        constraints: vec![],
        directive: Directive::Satisfy,
    };

    let contracts: Vec<_> = (0..200)
        .map(|i| Contract {
            predicates: vec![predicate.clone()],
            salt: [i as u8; 32],
        })
        .collect();

    let r = client
        .post(url.clone())
        .json(&sign(contracts[0].clone()))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.text().await.unwrap());

    let relayer = Relayer::new(server_address.as_str()).unwrap();
    let handle = relayer.run(conn).unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let result = essential_node_db::list_contracts(&test_conn, 0..3).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, 1);
    assert_eq!(result[0].1.len(), 1);
    assert_eq!(result[0].1[0].salt, [0; 32]);

    let r = client
        .post(url.clone())
        .json(&sign(contracts[1].clone()))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.text().await.unwrap());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let result = essential_node_db::list_contracts(&test_conn, 0..3).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[1].0, 2);
    assert_eq!(result[1].1.len(), 1);
    assert_eq!(result[1].1[0].salt, [1; 32]);

    handle.close().await.unwrap();

    for c in &contracts[2..] {
        let r = client
            .post(url.clone())
            .json(&sign(c.clone()))
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success(), "{}", r.text().await.unwrap());
    }
    let conn = rusqlite::Connection::open_with_flags("file::memory:", flags).unwrap();

    let relayer = Relayer::new(server_address.as_str()).unwrap();
    let handle = relayer.run(conn).unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let result = essential_node_db::list_contracts(&test_conn, 0..205).unwrap();
    assert_eq!(result.len(), 200);

    assert_eq!(result[1].0, 2);
    assert_eq!(result[1].1.len(), 1);
    assert_eq!(result[1].1[0].salt, [1; 32]);

    assert_eq!(result[2].0, 3);
    assert_eq!(result[2].1.len(), 1);
    assert_eq!(result[2].1[0].salt, [3; 32]);

    assert_eq!(result[199].0, 200);
    assert_eq!(result[199].1.len(), 1);
    assert_eq!(result[199].1[0].salt, [199; 32]);

    handle.close().await.unwrap();
}

pub async fn setup_server() -> (String, Child) {
    let mut child = Command::new("essential-rest-server")
        // .env("RUST_LOG", "info")
        .arg("--db")
        .arg("memory")
        .arg("0.0.0.0:0")
        .arg("--loop-freq")
        .arg("1")
        .arg("--disable-tracing")
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.stdout.take().unwrap();

    let buf = BufReader::new(stdout);
    let mut lines = buf.lines();

    let port;
    loop {
        if let Some(line) = lines.next_line().await.unwrap() {
            if line.contains("Listening") {
                port = line
                    .split(':')
                    .next_back()
                    .unwrap()
                    .trim()
                    .parse::<u16>()
                    .unwrap();
                break;
            }
        }
    }

    tokio::spawn(async move {
        loop {
            if let Some(line) = lines.next_line().await.unwrap() {
                println!("{}", line);
            }
        }
    });
    assert_ne!(port, 0);

    let server_address = format!("http://localhost:{}", port);
    (server_address, child)
}

fn sign(contract: Contract) -> SignedContract {
    let secp = secp256k1::Secp256k1::new();
    let key = secp.generate_keypair(&mut secp256k1::rand::rngs::OsRng).0;
    essential_sign::contract::sign(contract, &key)
}
