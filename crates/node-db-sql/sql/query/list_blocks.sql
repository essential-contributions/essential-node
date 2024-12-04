SELECT
    block.block_address,
    block.number,
    block.timestamp_secs,
    block.timestamp_nanos,
    solution_set.content_hash
FROM
    block
    LEFT JOIN block_solution_set ON block.id = block_solution_set.block_id
    LEFT JOIN solution_set ON block_solution_set.solution_set_id = solution_set.id
WHERE
    block.number >= :start_block AND block.number < :end_block
ORDER BY
    block.number ASC,
    block.block_address ASC,
    block_solution_set.solution_set_index ASC
