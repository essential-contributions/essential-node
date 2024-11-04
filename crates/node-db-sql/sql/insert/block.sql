INSERT
    OR IGNORE INTO block (
        block_address,
        parent_block_id,
        number,
        timestamp_secs,
        timestamp_nanos
    )
VALUES
    (
        :block_address,
        (
            SELECT
                COALESCE(
                    (
                        SELECT
                            id
                        FROM
                            block
                        WHERE
                            block_address = :parent_block_address
                    ),
                    0 -- Default to for big bang block to it's own parent
                )
        ),
        :number,
        :timestamp_secs,
        :timestamp_nanos
    );