SELECT
    contract.da_block_number,
    contract.salt,
    contract.content_hash,
    predicate.predicate
FROM
    contract
    JOIN contract_predicate ON contract.id = contract_predicate.contract_id
    JOIN predicate ON contract_predicate.predicate_id = predicate.id
WHERE
    contract.da_block_number >= :start_block AND contract.da_block_number < :end_block
ORDER BY
    contract.da_block_number ASC, contract_predicate.id ASC;
