CREATE TABLE IF NOT EXISTS finalized_block (
    block_number INTEGER PRIMARY KEY,
    block_id BLOB NOT NULL UNIQUE,
    FOREIGN KEY (block_number) REFERENCES block (number),
    FOREIGN KEY (block_id) REFERENCES block (id)
);
