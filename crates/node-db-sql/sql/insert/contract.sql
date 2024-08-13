INSERT
    OR IGNORE INTO contract (salt, content_hash, l2_block_number)
VALUES
    (:salt, :content_hash, :l2_block_number)
