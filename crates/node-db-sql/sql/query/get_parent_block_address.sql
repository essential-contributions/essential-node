SELECT
    block_address
FROM
    block
WHERE
    id = (
        SELECT
            parent_block_id
        FROM
            block
        WHERE
            block_address = :block_address
        LIMIT
            1
    )
LIMIT
    1