INSERT
    OR IGNORE INTO contract (
        content_hash,
        salt,
        created_at_seconds,
        created_at_nanos
    )
VALUES
    (:content_hash, :salt, :created_at_seconds, :created_at_nanos)
