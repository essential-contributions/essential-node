INSERT
    OR IGNORE INTO contracts (
        content_hash,
        salt,
        signature,
        created_at_seconds,
        created_at_nanos
    )
VALUES
    (?, ?, ?, ?, ?)