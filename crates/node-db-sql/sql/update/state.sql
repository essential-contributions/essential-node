INSERT INTO state (contract_ca, key, value)
VALUES (:contract_ca, :key, :value)
ON CONFLICT (contract_ca, key) DO UPDATE SET value = EXCLUDED.value;
