CREATE TABLE IF NOT EXISTS block_solution (
    block_number INTEGER NOT NULL,
    solution_id INTEGER NOT NULL,
    solution_index INTEGER NOT NULL,
    FOREIGN KEY (block_number) REFERENCES block (number),
    FOREIGN KEY (solution_id) REFERENCES solution (id),
    PRIMARY KEY (block_number, solution_id, solution_index)
);
