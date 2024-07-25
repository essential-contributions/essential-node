SELECT
    predicates.predicate
FROM
    contract_pairing
    JOIN contracts ON contract_pairing.contract_id = contracts.id
    JOIN predicates ON contract_pairing.predicate_id = predicates.id
WHERE
    contracts.content_hash = ?
ORDER BY
    contract_pairing.id ASC;