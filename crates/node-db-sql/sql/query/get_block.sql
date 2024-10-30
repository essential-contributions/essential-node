SELECT
    block.number,
    block.timestamp_secs,
    block.timestamp_nanos,
    solution.solution
FROM
    block
    LEFT JOIN block_solution ON block.id = block_solution.block_id
    LEFT JOIN solution ON block_solution.solution_id = solution.id
WHERE
    block.block_address = :block_address
