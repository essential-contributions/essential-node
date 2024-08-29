SELECT
    block.number,
    solution.content_hash
FROM
    failed_solution
    LEFT JOIN block ON failed_solution.block_id = block.id
    LEFT JOIN solution ON failed_solution.solution_id = solution.id
ORDER BY
    block.number ASC,
    solution.content_hash ASC;
