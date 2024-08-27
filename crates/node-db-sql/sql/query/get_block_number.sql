SELECT
    block.number
FROM
    block
WHERE
    block.block_address = :block_address
LIMIT
    1;