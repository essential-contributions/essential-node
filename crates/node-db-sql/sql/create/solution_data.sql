CREATE TABLE IF NOT EXISTS solution_data (
    id INTEGER PRIMARY KEY,
    solution_id INTEGER NOT NULL,
    data_index INTEGER NOT NULL,
    contract_addr BLOB NOT NULL,
    predicate_addr BLOB NOT NULL,
    FOREIGN KEY (solution_id) REFERENCES solution (id)
    UNIQUE (solution_id, data_index)
);
