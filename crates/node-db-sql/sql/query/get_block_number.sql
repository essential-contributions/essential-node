SELECT
    block.number
FROM
    block
WHERE
    block.block_hash = :block_hash
LIMIT
    1;