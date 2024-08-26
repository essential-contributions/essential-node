SELECT
    block.block_hash
FROM
    block
    JOIN finalized_block ON block.id = finalized_block.block_id
ORDER BY
    block.number DESC
LIMIT
    1;