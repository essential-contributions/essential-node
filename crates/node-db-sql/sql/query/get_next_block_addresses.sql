SELECT
    b1.block_address
FROM
    block b1
    LEFT JOIN block b2 ON b1.number = b2.number + 1
WHERE
    b2.block_address = :current_block
