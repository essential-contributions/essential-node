CREATE TABLE IF NOT EXISTS solutions (
    id INTEGER PRIMARY KEY,
    content_hash BLOB NOT NULL UNIQUE,
    solution BLOB NOT NULL
);