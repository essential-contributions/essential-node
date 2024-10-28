SELECT
    block.block_address
FROM
    block
WHERE
    block.number = 0
LIMIT
    1
