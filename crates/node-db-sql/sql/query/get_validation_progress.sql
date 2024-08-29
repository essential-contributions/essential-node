SELECT
    block.block_address
FROM
    block
    JOIN validation_progress ON block.id = validation_progress.block_id
LIMIT
    1