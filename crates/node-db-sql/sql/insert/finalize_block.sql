INSERT
    OR ABORT INTO finalized_block (block_number, block_id)
SELECT
    number,
    id
FROM
    block
WHERE
    block.block_address = :block_address
LIMIT
    1;