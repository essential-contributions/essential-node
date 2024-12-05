CREATE TABLE IF NOT EXISTS pred_data (
    id INTEGER PRIMARY KEY,
    solution_id INTEGER NOT NULL,
    pred_data_index INTEGER NOT NULL,
    value BLOB NOT NULL,
    FOREIGN KEY (solution_id) REFERENCES solution (id)
);
