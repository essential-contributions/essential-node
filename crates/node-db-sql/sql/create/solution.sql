CREATE TABLE IF NOT EXISTS solution (
    id INTEGER PRIMARY KEY,
    content_hash BLOB NOT NULL UNIQUE,
    solution BLOB NOT NULL
);