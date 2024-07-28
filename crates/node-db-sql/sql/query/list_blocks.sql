SELECT
    block.number,
    block.created_at_seconds,
    block.created_at_nanos,
    solution.solution
FROM
    block
    LEFT JOIN block_solution ON block.number = block_solution.block_number
    LEFT JOIN solution ON block_solution.content_hash = solution.content_hash
WHERE
    block.number >= :start_block AND block.number < :end_block
ORDER BY
    block.number ASC,
    block_solution.solution_index ASC
