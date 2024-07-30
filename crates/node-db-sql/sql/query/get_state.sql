SELECT state.value
FROM state
JOIN contract ON state.contract_id = contract.id
WHERE contract.content_hash = ? AND state.key = ?;
