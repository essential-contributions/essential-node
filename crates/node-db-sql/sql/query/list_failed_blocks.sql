SELECT
    block.number,
    solution_set.content_hash
FROM
    failed_block
    JOIN block ON failed_block.block_id = block.id
    JOIN solution_set ON failed_block.solution_set_id = solution_set.id
WHERE
    block.number >= :start_block AND block.number < :end_block
ORDER BY
    block.number ASC,
    block.block_address ASC,
    solution_set.content_hash ASC;
