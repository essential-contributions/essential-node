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
    (
        block.timestamp_secs >= :start_secs
        OR (
            block.timestamp_secs = :start_secs
            AND block.timestamp_nanos >= :start_nanos
        )
    )
    AND (
        block.timestamp_secs < :end_secs
        OR (
            block.timestamp_secs = :end_secs
            AND block.timestamp_nanos < :end_nanos
        )
    )
ORDER BY
    block.number ASC,
    block.block_address ASC,
    block_solution.solution_index ASC
LIMIT
    :page_size OFFSET :page_number * :page_size;
