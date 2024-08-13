CREATE TABLE IF NOT EXISTS contract (
    id INTEGER PRIMARY KEY,
    l2_block_number INTEGER NOT NULL,
    salt BLOB NOT NULL,
    content_hash BLOB NOT NULL UNIQUE
);
