SELECT
    l2_block_number,
    content_hash
FROM
    contract_progress
WHERE
    id = 1
LIMIT
    1;