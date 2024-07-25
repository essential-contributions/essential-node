CREATE TABLE IF NOT EXISTS contracts (
    id INTEGER PRIMARY KEY,
    content_hash BLOB NOT NULL UNIQUE,
    salt BLOB NOT NULL,
    signature BLOB NOT NULL,
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL
);