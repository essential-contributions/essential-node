SELECT
    solution.content_hash
FROM
    block
    LEFT JOIN block_solution ON block.id = block_solution.block_id
    LEFT JOIN solution ON block_solution.solution_id = solution.id
WHERE
    block.block_address = :block_address
ORDER BY
    block_solution.solution_index ASC
