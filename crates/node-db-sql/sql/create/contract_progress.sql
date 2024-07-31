CREATE TABLE IF NOT EXISTS contract_progress (
    id INTEGER PRIMARY KEY,
    logical_clock INTEGER NOT NULL,
    content_hash BLOB NOT NULL
);
