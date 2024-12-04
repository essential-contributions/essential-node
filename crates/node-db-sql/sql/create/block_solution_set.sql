CREATE TABLE IF NOT EXISTS block_solution_set (
    id INTEGER PRIMARY KEY,
    block_id INTEGER NOT NULL,
    solution_set_id INTEGER NOT NULL,
    solution_set_index INTEGER NOT NULL,
    FOREIGN KEY (block_id) REFERENCES block (id),
    FOREIGN KEY (solution_set_id) REFERENCES solution_set (id),
    UNIQUE(block_id, solution_set_id, solution_set_index)
);
