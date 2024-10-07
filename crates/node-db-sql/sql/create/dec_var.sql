CREATE TABLE IF NOT EXISTS dec_var (
    id INTEGER PRIMARY KEY,
    solution_id INTEGER NOT NULL,
    data_index INTEGER NOT NULL,
    dec_var_index INTEGER NOT NULL,
    value BLOB NOT NULL
);