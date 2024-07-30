CREATE TABLE IF NOT EXISTS block (
    number INTEGER PRIMARY KEY,
    timestamp_secs INTEGER NOT NULL,
    timestamp_nanos INTEGER NOT NULL
);
