INSERT OR IGNORE INTO
    block_solution (block_number, solution_id, solution_index)
VALUES
    (
        :block_number,
        (SELECT id FROM solution WHERE solution.content_hash = :solution_hash LIMIT 1),
        :solution_index
    );
