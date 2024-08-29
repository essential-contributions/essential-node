INSERT
    OR REPLACE INTO state_progress (id, block_id)
VALUES
    (
        1,
        (
            SELECT
                id
            FROM
                block
            WHERE
                block.block_address = :block_address
        )
    );