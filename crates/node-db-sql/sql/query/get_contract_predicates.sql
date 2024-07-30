SELECT
    predicate.predicate
FROM
    contract_predicate
    JOIN contract ON contract_predicate.contract_id = contract.id
    JOIN predicate ON contract_predicate.predicate_id = predicate.id
WHERE
    contract.content_hash = :contract_hash
ORDER BY
    contract_predicate.id ASC;
