
SELECT
    block.number,
    block.timestamp_secs,
    block.timestamp_nanos
FROM
    block
WHERE
    block.block_address = :block_address
LIMIT
    1;
