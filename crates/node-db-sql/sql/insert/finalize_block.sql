INSERT
    OR IGNORE INTO finalized_block (block_number, block_id)
SELECT
    number,
    id
FROM
    block
WHERE
    block.block_hash = :block_hash
LIMIT
    1;