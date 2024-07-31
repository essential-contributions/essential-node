SELECT
    logical_clock,
    content_hash
FROM
    contract_progress
WHERE
    id = 1
LIMIT
    1;