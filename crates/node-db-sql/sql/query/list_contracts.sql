-- Step 1: Retrieve the unique contract_id values
WITH unique_contract_ids AS (
    SELECT DISTINCT contract_id
    FROM contract_predicate
    ORDER BY contract_id
    LIMIT :page_size OFFSET :page_size * :page_number
)
-- Step 2: Retrieve predicates for the contracts in the range
SELECT
    isp.contract_id,
    i.predicate
FROM
    contract_predicate isp
    JOIN unique_contract_ids usi ON isp.contract_id = usi.contract_id
    JOIN predicates i ON isp.predicate_id = i.id
ORDER BY
    isp.contract_id,
    isp.id;
