INSERT
    OR IGNORE INTO solution (solution_set_id, solution_index, contract_addr, predicate_addr)
VALUES
    (
        (
            SELECT
                id
            FROM
                solution_set
            WHERE
                content_addr = :solution_set_addr
            LIMIT
                1
        ), :solution_index, :contract_addr, :predicate_addr
    )
