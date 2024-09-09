SELECT
    block.block_address
FROM
    block
    JOIN state_progress ON block.id = state_progress.block_id
LIMIT
    1