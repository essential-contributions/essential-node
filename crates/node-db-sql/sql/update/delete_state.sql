DELETE FROM
    state
WHERE
    contract_ca = :contract_ca
    AND key = :key;
