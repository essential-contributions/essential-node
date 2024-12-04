CREATE TABLE IF NOT EXISTS mutation (
    id INTEGER PRIMARY KEY,
    solution_id INTEGER NOT NULL,
    mutation_index INTEGER NOT NULL,
    key BLOB NOT NULL,
    value BLOB NOT NULL,
    FOREIGN KEY (solution_id) REFERENCES solution (id)
);
