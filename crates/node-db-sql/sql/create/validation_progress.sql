CREATE TABLE IF NOT EXISTS validation_progress (
    id INTEGER PRIMARY KEY,
    number INTEGER NOT NULL,
    block_address BLOB NOT NULL
);
