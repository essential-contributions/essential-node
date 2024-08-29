CREATE TABLE IF NOT EXISTS failed_solution (
    id INTEGER PRIMARY KEY,
    block_id BLOB NOT NULL,
    solution_id BLOB NOT NULL,
    FOREIGN KEY (block_id) REFERENCES block (id)
    FOREIGN KEY (solution_id) REFERENCES solution (id)
);
