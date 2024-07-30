INSERT INTO state (contract_id, key, value)
SELECT id, :key, :value
FROM contract
WHERE content_hash = :contract_hash
ON CONFLICT (contract_id, key) DO UPDATE SET value = EXCLUDED.value;
