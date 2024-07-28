CREATE TABLE IF NOT EXISTS predicate (
    id INTEGER PRIMARY KEY,
    content_hash BLOB NOT NULL UNIQUE,
    predicate BLOB NOT NULL
);
