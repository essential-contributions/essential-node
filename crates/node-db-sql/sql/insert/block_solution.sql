INSERT OR IGNORE INTO
    block_solution (block_id, solution_id, solution_index)
VALUES
    (
        (SELECT id FROM block WHERE block.block_hash = :block_hash LIMIT 1),
        (SELECT id FROM solution WHERE solution.content_hash = :solution_hash LIMIT 1),
        :solution_index
    );
