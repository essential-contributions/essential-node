INSERT
    OR IGNORE INTO block (block_address, number, timestamp_secs, timestamp_nanos)
VALUES
    (:block_address, :number, :timestamp_secs, :timestamp_nanos);
