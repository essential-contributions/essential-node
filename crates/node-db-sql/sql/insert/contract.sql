INSERT
    OR IGNORE INTO contract (salt, content_hash, da_block_number)
VALUES
    (:salt, :content_hash, :da_block_number)
