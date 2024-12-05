SELECT
    solution_set.content_hash
FROM
    block
    LEFT JOIN block_solution_set ON block.id = block_solution_set.block_id
    LEFT JOIN solution_set ON block_solution_set.solution_set_id = solution_set.id
WHERE
    block.block_address = :block_address
ORDER BY
    block_solution_set.solution_set_index ASC
