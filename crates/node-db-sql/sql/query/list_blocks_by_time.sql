SELECT
    block.block_address,
    block.number,
    block.timestamp_secs,
    block.timestamp_nanos,
    solution_set.content_addr

FROM
    block
    LEFT JOIN block_solution_set ON block.id = block_solution_set.block_id
    LEFT JOIN solution_set ON block_solution_set.solution_set_id = solution_set.id
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
    block_solution_set.solution_set_index ASC
LIMIT
    :page_size OFFSET :page_number * :page_size;
