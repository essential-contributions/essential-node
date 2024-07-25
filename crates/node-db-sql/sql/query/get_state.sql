SELECT state.value
FROM state
JOIN contracts ON state.contract_id = contracts.id
WHERE contracts.content_hash = ? AND state.key = ?;
