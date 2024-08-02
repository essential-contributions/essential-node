use essential_relayer::{DataSyncError, GetConn, Relayer};
use essential_types::{
    contract::{Contract, SignedContract},
    predicate::{Directive, Predicate},
    solution::{Mutation, Solution, SolutionData},
    Block, PredicateAddress,
};
use reqwest::ClientBuilder;
use rusqlite::OpenFlags;
use std::{future::Future, process::Stdio};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

#[tokio::test]
async fn test_sync() {
    let (server_address, mut child) = setup_server().await;

    let client = ClientBuilder::new()
        .http2_prior_knowledge()
        .build()
        .unwrap();
    let url = reqwest::Url::parse(server_address.as_str())
        .unwrap()
        .join("/deploy-contract")
        .unwrap();
    let solve_url = reqwest::Url::parse(server_address.as_str())
        .unwrap()
        .join("/submit-solution")
        .unwrap();

    let mut flags = OpenFlags::default();
    flags.insert(OpenFlags::SQLITE_OPEN_SHARED_CACHE);
    let mut test_conn = rusqlite::Connection::open_with_flags("file::memory:", flags).unwrap();
    let tx = test_conn.transaction().unwrap();
    essential_node_db::create_tables(&tx).unwrap();
    tx.commit().unwrap();

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
    let solutions: Vec<_> = contracts
        .iter()
        .map(|c| {
            let contract = essential_hash::contract_addr::from_contract(c);
            let predicate = essential_hash::content_addr(&c.predicates[0]);
            let addr = PredicateAddress {
                contract,
                predicate,
            };
            Solution {
                data: vec![SolutionData {
                    predicate_to_solve: addr,
                    decision_variables: vec![],
                    transient_data: vec![],
                    state_mutations: vec![Mutation {
                        key: vec![1],
                        value: vec![1],
                    }],
                }],
            }
        })
        .collect();

    let r = client
        .post(url.clone())
        .json(&sign(contracts[0].clone()))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.text().await.unwrap());

    let r = client
        .post(solve_url.clone())
        .json(&solutions[0])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.text().await.unwrap());

    let relayer = Relayer::new(server_address.as_str()).unwrap();
    let (block_notify, mut new_block) = tokio::sync::watch::channel(());
    let (contract_notify, mut new_contract) = tokio::sync::watch::channel(());
    let handle = relayer.run(Conn, contract_notify, block_notify).unwrap();

    new_contract.changed().await.unwrap();
    let result = essential_node_db::list_contracts(&test_conn, 0..3).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, 0);
    assert_eq!(result[0].1.len(), 1);
    assert_eq!(result[0].1[0].salt, [0; 32]);

    new_block.changed().await.unwrap();
    let result = essential_node_db::list_blocks(&test_conn, 0..100).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].number, 0);
    assert_eq!(result[0].solutions.len(), 1);
    assert_eq!(result[0].solutions[0], solutions[0]);

    let r = client
        .post(url.clone())
        .json(&sign(contracts[1].clone()))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.text().await.unwrap());

    new_contract.changed().await.unwrap();
    let result = essential_node_db::list_contracts(&test_conn, 0..3).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[1].0, 1);
    assert_eq!(result[1].1.len(), 1);
    assert_eq!(result[1].1[0].salt, [1; 32]);

    let r = client
        .post(solve_url.clone())
        .json(&solutions[1])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.text().await.unwrap());

    new_block.changed().await.unwrap();
    let result = essential_node_db::list_blocks(&test_conn, 0..100).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[1].number, 1);
    assert_eq!(result[1].solutions.len(), 1);
    assert_eq!(result[1].solutions[0], solutions[1]);

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

    for s in &solutions[2..] {
        let r = client.post(solve_url.clone()).json(s).send().await.unwrap();
        assert!(r.status().is_success(), "{}", r.text().await.unwrap());
    }

    let relayer = Relayer::new(server_address.as_str()).unwrap();
    let (block_notify, _new_block) = tokio::sync::watch::channel(());
    let (contract_notify, mut new_contract) = tokio::sync::watch::channel(());
    let handle = relayer.run(Conn, contract_notify, block_notify).unwrap();

    wait_for(&mut new_contract).await;

    let result = essential_node_db::list_contracts(&test_conn, 0..205).unwrap();
    assert_eq!(result.len(), 200);

    assert_eq!(result[1].0, 1);
    assert_eq!(result[1].1.len(), 1);
    assert_eq!(result[1].1[0].salt, [1; 32]);

    assert_eq!(result[2].0, 2);
    assert_eq!(result[2].1.len(), 1);
    assert_eq!(result[2].1[0].salt, [2; 32]);

    assert_eq!(result[199].0, 199);
    assert_eq!(result[199].1.len(), 1);
    assert_eq!(result[199].1[0].salt, [199; 32]);

    let start = tokio::time::Instant::now();
    let mut num_solutions: usize;
    let mut result: Vec<Block>;
    loop {
        if start.elapsed() > tokio::time::Duration::from_secs(10) {
            panic!("timeout");
        }
        let Ok(r) = essential_node_db::list_blocks(&test_conn, 0..203) else {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            continue;
        };
        result = r;
        num_solutions = result.iter().map(|b| b.solutions.len()).sum();
        if num_solutions >= 200 {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    assert_eq!(num_solutions, 200);
    assert!(result
        .iter()
        .zip(result.iter().skip(1))
        .all(|(a, b)| a.number + 1 == b.number));

    let num_blocks = result.len();

    handle.close().await.unwrap();
    child.kill().await.unwrap();

    let (server_address, _child) = setup_server().await;

    let relayer = Relayer::new(server_address.as_str()).unwrap();
    let (block_notify, _new_block) = tokio::sync::watch::channel(());
    let (contract_notify, _new_contract) = tokio::sync::watch::channel(());
    let handle = relayer.run(Conn, contract_notify, block_notify).unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let r = handle.close().await;
    assert!(
        matches!(
            r,
            Err(essential_relayer::Error::DataSyncFailed(
                DataSyncError::Fork(i, _, None)
            )) if i == (num_blocks - 1) as u64
        ) || matches!(
            r,
            Err(essential_relayer::Error::DataSyncFailed(
                DataSyncError::ContractMismatch(199, _, None)
            ))
        ),
        "{} {:?}",
        num_blocks,
        r
    );
}

#[derive(Clone, Copy)]
struct Conn;

impl GetConn for Conn {
    type Error = rusqlite::Error;
    type Connection = rusqlite::Connection;

    fn get(
        &self,
    ) -> impl Future<Output = std::result::Result<Self::Connection, Self::Error>> + Send {
        let mut flags = OpenFlags::default();
        flags.insert(OpenFlags::SQLITE_OPEN_SHARED_CACHE);
        let r = rusqlite::Connection::open_with_flags("file::memory:", flags).map_err(Into::into);
        futures::future::ready(r)
    }
}

async fn wait_for(notify: &mut tokio::sync::watch::Receiver<()>) {
    loop {
        if tokio::time::timeout(tokio::time::Duration::from_millis(10), notify.changed())
            .await
            .is_err()
        {
            break;
        }
    }
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
