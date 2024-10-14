INSERT
    OR IGNORE INTO contract (l2_block_number, salt, content_hash)
VALUES
    (:l2_block_number, :salt, :content_hash)