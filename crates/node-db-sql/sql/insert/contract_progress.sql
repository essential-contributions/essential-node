INSERT
    OR REPLACE INTO contract_progress (id, l2_block_number, content_hash)
VALUES
    (1, :l2_block_number, :content_hash)
