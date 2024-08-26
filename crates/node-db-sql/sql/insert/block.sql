INSERT
    OR IGNORE INTO block (block_hash, number, timestamp_secs, timestamp_nanos)
VALUES
    (:block_hash, :number, :timestamp_secs, :timestamp_nanos);
