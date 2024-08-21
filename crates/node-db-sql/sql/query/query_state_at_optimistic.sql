SELECT
    mutation.value
FROM
    mutation
    JOIN block_solution ON block_solution.solution_id = mutation.solution_id
WHERE
    mutation.contract_ca = :contract_ca
    AND mutation.key = :key
    AND (
        block_solution.block_number < :block_number
        OR (
            block_solution.block_number = :block_number
            AND block_solution.solution_index <= :solution_index
        )
    )
ORDER BY
    block_solution.block_number DESC,
    block_solution.solution_index DESC
LIMIT
    1;