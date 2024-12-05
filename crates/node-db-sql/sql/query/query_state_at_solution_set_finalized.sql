SELECT
    mutation.value
FROM
    mutation
    JOIN solution ON solution.id = mutation.solution_id
    JOIN block_solution_set ON block_solution_set.solution_set_id = solution.solution_set_id
    JOIN finalized_block ON finalized_block.block_id = block_solution_set.block_id
WHERE
    solution.contract_addr = :contract_ca
    AND mutation.key = :key
    AND (
        finalized_block.block_number,
        block_solution_set.solution_set_index
    ) <= (:block_number, :solution_set_index)
ORDER BY
    finalized_block.block_number DESC,
    block_solution_set.solution_set_index DESC
LIMIT
    1;
