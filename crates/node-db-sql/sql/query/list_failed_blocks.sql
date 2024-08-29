SELECT
    block.number,
    solution.content_hash
FROM
    failed_block
    JOIN block ON failed_block.block_id = block.id
    JOIN solution ON failed_block.solution_id = solution.id
ORDER BY
    block.number ASC,
    block.block_address ASC,
    solution.content_hash ASC;
