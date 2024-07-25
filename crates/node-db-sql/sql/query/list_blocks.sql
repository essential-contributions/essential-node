SELECT
    solved.block_id,
    solutions.solution,
    block.created_at_seconds,
    block.created_at_nanos
FROM
    solved
    JOIN solutions ON solved.content_hash = solutions.content_hash
    JOIN block ON solved.block_id = block.id
WHERE
    block_id IN (
        SELECT
            id
        FROM
            block
        ORDER BY
            id ASC
        LIMIT
            :page_size OFFSET :page_number * :page_size
    )
ORDER BY
    block_id ASC;
