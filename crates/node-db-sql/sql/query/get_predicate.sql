SELECT
    predicates.predicate
FROM
    predicates 
    JOIN contract_pairing ON predicates.id = contract_pairing.predicate_id
    JOIN contracts ON contracts.id = contract_pairing.contract_id
WHERE
    contracts.content_hash = ?
    AND predicates.content_hash = ?;