DELETE FROM
    state
WHERE
    contract_id = (
        SELECT
            contract.id
        FROM
            contract
        WHERE
            contract.content_hash = :contract_hash
    )
    AND KEY = :key;
