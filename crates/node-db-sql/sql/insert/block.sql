INSERT
    OR IGNORE INTO block (number, timestamp_secs, timestamp_nanos)
VALUES
    (:number, :timestamp_secs, :timestamp_nanos);
