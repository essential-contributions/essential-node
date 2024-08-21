CREATE TABLE IF NOT EXISTS mutation (
    id INTEGER PRIMARY KEY,
    solution_id INTEGER NOT NULL,
    data_index INTEGER NOT NULL,
    mutation_index INTEGER NOT NULL,
    contract_ca BLOB NOT NULL,
    key BLOB NOT NULL,
    value BLOB NOT NULL
);