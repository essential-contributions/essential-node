INSERT 
    OR IGNORE INTO failed_block (block_id, solution_id)
VALUES
    (
        (SELECT id FROM block WHERE block.block_address = :block_address LIMIT 1),
        (SELECT id FROM solution WHERE solution.content_hash = :solution_hash LIMIT 1)
    );
