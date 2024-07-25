CREATE TABLE IF NOT EXISTS predicates (
    id INTEGER PRIMARY KEY,
    predicate BLOB NOT NULL,
    content_hash BLOB NOT NULL UNIQUE
);