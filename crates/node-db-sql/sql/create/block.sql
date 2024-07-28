CREATE TABLE IF NOT EXISTS block (
    number INTEGER PRIMARY KEY,
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL
);
