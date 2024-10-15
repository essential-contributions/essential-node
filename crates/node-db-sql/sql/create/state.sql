CREATE TABLE IF NOT EXISTS state (
    id INTEGER PRIMARY KEY,
    contract_ca BLOB NOT NULL,
    key BLOB NOT NULL,
    value BLOB NOT NULL,
    UNIQUE(contract_ca, key)
);
