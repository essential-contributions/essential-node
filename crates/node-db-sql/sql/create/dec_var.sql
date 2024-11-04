CREATE TABLE IF NOT EXISTS dec_var (
    id INTEGER PRIMARY KEY,
    data_id INTEGER NOT NULL,
    dec_var_index INTEGER NOT NULL,
    value BLOB NOT NULL,
    FOREIGN KEY (data_id) REFERENCES solution_data (id)
);
