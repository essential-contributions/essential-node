INSERT
    OR IGNORE INTO solution_data (solution_id, data_index, contract_addr, predicate_addr)
VALUES
    (
        (
            SELECT
                id
            FROM
                solution
            WHERE
                content_hash = :solution_hash
            LIMIT
                1
        ), :data_index, :contract_addr, :predicate_addr
    )