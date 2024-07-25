INSERT INTO state (contract_id, key, value)
SELECT id, ?, ?
FROM contracts
WHERE content_hash = ?
ON CONFLICT (contract_id, key) DO UPDATE SET value = EXCLUDED.value;
