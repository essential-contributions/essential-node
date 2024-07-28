INSERT
    OR IGNORE INTO block (number, created_at_seconds, created_at_nanos)
VALUES
    (:number, :created_at_seconds, :created_at_nanos);
