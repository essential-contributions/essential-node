CREATE TABLE IF NOT EXISTS contract_predicate (
    id INTEGER PRIMARY KEY,
    contract_id INTEGER NOT NULL,
    predicate_id INTEGER NOT NULL,
    FOREIGN KEY (contract_id) REFERENCES contract (id),
    FOREIGN KEY (predicate_id) REFERENCES predicate (id),
    UNIQUE(contract_id, predicate_id)
);
