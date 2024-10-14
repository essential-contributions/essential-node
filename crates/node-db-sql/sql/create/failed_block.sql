CREATE TABLE IF NOT EXISTS failed_block (
    id INTEGER PRIMARY KEY,
    block_id INTEGER NOT NULL,
    solution_id INTEGER NOT NULL,
    FOREIGN KEY (block_id) REFERENCES block (id),
    FOREIGN KEY (solution_id) REFERENCES solution (id),
    UNIQUE (block_id, solution_id)
);