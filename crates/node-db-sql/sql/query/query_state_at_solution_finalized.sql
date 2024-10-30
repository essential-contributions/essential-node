SELECT
    mutation.value
FROM
    mutation
    JOIN solution_data ON solution_data.id = mutation.data_id
    JOIN block_solution ON block_solution.solution_id = mutation.solution_id
    JOIN finalized_block ON finalized_block.block_id = block_solution.block_id
WHERE
    solution_data.contract_addr = :contract_ca
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