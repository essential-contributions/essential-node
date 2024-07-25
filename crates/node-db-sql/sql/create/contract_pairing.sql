CREATE TABLE IF NOT EXISTS contract_pairing (
    id INTEGER PRIMARY KEY,
    contract_id INTEGER NOT NULL,
    predicate_id INTEGER NOT NULL,
    FOREIGN KEY (contract_id) REFERENCES contracts (id),
    FOREIGN KEY (predicate_id) REFERENCES predicates (id),
    UNIQUE(contract_id, predicate_id)
);