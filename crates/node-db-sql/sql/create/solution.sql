CREATE TABLE IF NOT EXISTS solution (
    id INTEGER PRIMARY KEY,
    solution_set_id INTEGER NOT NULL,
    solution_index INTEGER NOT NULL,
    contract_addr BLOB NOT NULL,
    predicate_addr BLOB NOT NULL,
    FOREIGN KEY (solution_set_id) REFERENCES solution_set (id)
    UNIQUE (solution_set_id, solution_index)
);
