CREATE TABLE IF NOT EXISTS deployed_contract (
    id INTEGER PRIMARY KEY,
    contract_hash BLOB NOT NULL UNIQUE,
    mutation_id INTEGER NOT NULL UNIQUE,
    FOREIGN KEY (mutation_id) REFERENCES mutation (id)
);