INSERT OR IGNORE INTO
    block_solution_set (block_id, solution_set_id, solution_set_index)
VALUES
    (
        (SELECT id FROM block WHERE block.block_address = :block_address LIMIT 1),
        (SELECT id FROM solution_set WHERE solution_set.content_hash = :solution_set_hash LIMIT 1),
        :solution_set_index
    );
