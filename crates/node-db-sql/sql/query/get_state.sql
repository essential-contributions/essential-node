SELECT state.value
FROM state
WHERE state.contract_ca = ? AND state.key = ?;
