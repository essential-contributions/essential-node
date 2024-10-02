CREATE TABLE IF NOT EXISTS pub_var (
    id INTEGER PRIMARY KEY,
    solution_id INTEGER NOT NULL,
    data_index INTEGER NOT NULL,
    key BLOB NOT NULL,
    value BLOB NOT NULL
);