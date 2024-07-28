SELECT
    predicate.predicate
FROM
    predicate
    JOIN contract_predicate ON predicate.id = contract_predicate.predicate_id
    JOIN contract ON contract.id = contract_predicate.contract_id
WHERE
    contract.content_hash = ?
    AND predicate.content_hash = ?;
