SELECT
    block.block_address,
    block.number,
    block.timestamp_secs,
    block.timestamp_nanos,
    solution.solution
FROM
    block
    LEFT JOIN block_solution ON block.id = block_solution.block_id
    LEFT JOIN solution ON block_solution.solution_id = solution.id
WHERE
    block.number >= :start_block AND block.number < :end_block
    AND
    NOT EXISTS (SELECT 1 FROM failed_block WHERE block_id = block.id)
ORDER BY
    block.number ASC,
    block.block_address ASC,
    block_solution.solution_index ASC
