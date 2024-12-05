INSERT
    OR IGNORE INTO failed_block (block_id, solution_set_id)
VALUES
    (
        (SELECT id FROM block WHERE block.block_address = :block_address LIMIT 1),
        (SELECT id FROM solution_set WHERE solution_set.content_addr = :solution_set_addr LIMIT 1)
    );
