CREATE TABLE IF NOT EXISTS contract_progress (
    id INTEGER PRIMARY KEY,
    l2_block_number INTEGER NOT NULL,
    content_hash BLOB NOT NULL
);
