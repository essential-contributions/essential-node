CREATE TABLE IF NOT EXISTS block_solution (
    id INTEGER PRIMARY KEY,
    block_id INTEGER NOT NULL,
    solution_id INTEGER NOT NULL,
    solution_index INTEGER NOT NULL,
    FOREIGN KEY (block_id) REFERENCES block (id),
    FOREIGN KEY (solution_id) REFERENCES solution (id),
    UNIQUE(block_id, solution_id, solution_index)
);
