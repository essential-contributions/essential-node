CREATE TABLE IF NOT EXISTS validation_progress (
    id INTEGER PRIMARY KEY,
    block_id INTEGER NOT NULL,
    FOREIGN KEY (block_id) REFERENCES block (id)
);