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
    (
        block.created_at_seconds > :start_seconds
        OR (
            block.created_at_seconds = :start_seconds
            AND block.created_at_nanos >= :start_nanos
        )
    )
    AND (
        block.created_at_seconds < :end_seconds
        OR (
            block.created_at_seconds = :end_seconds
            AND block.created_at_nanos <= :end_nanos
        )
    )
ORDER BY
    block.number ASC,
    block_solution.solution_index ASC
LIMIT
    :page_size OFFSET :page_number * :page_size;
