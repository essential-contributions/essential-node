CREATE TABLE IF NOT EXISTS state_progress (
    id INTEGER PRIMARY KEY,
    number INTEGER NOT NULL,
    block_hash BLOB NOT NULL
);
