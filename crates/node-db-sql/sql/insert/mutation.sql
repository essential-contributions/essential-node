INSERT
    OR IGNORE INTO mutation (
        solution_id,
        mutation_index,
        key,
        value
    )
VALUES
    (
       (
            SELECT
                solution.id
            FROM
                solution
                JOIN solution_set ON solution_set.id = solution.solution_set_id
            WHERE
                solution_set.content_addr = :solution_set_addr AND solution.solution_index = :solution_index
            LIMIT
                1
        ), :mutation_index, :key, :value
    );
