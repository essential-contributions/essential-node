INSERT OR IGNORE INTO
    contract_pairing (contract_id, predicate_id)
VALUES
    (
        (
            SELECT
                id
            FROM
                contracts
            WHERE
                content_hash = ?
            LIMIT
                1
        ), (
            SELECT
                id
            FROM
               predicates 
            WHERE
                content_hash = ?
            LIMIT
                1
        )
    );