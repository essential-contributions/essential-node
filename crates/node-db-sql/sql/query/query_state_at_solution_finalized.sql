SELECT
    mutation.value
FROM
    mutation
    JOIN block_solution ON block_solution.solution_id = mutation.solution_id
    JOIN finalized_block ON finalized_block.block_id = block_solution.block_id
WHERE
    mutation.contract_ca = :contract_ca
    AND mutation.key = :key
    AND (
        finalized_block.block_number,
        block_solution.solution_index
    ) <= (:block_number, :solution_index)
ORDER BY
    finalized_block.block_number DESC,
    block_solution.solution_index DESC
LIMIT
    1;