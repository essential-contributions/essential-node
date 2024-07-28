INSERT OR IGNORE INTO
    contract_predicate (contract_id, predicate_id)
VALUES
    (
        (SELECT id FROM contract WHERE content_hash = :contract_hash LIMIT 1),
        (SELECT id FROM predicate WHERE content_hash = :predicate_hash LIMIT 1)
    );
