CREATE TABLE IF NOT EXISTS block (
    id INTEGER PRIMARY KEY,
    block_address BLOB NOT NULL UNIQUE,
    parent_block_id INTEGER NOT NULL,
    number INTEGER NOT NULL,
    timestamp_secs INTEGER NOT NULL,
    timestamp_nanos INTEGER NOT NULL
);
