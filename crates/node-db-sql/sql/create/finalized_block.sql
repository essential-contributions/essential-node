CREATE TABLE IF NOT EXISTS finalized_block (
    block_number INTEGER PRIMARY KEY,
    block_id INTEGER NOT NULL UNIQUE,
    FOREIGN KEY (block_id) REFERENCES block (id)
);
