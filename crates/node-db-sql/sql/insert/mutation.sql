INSERT
    OR IGNORE INTO mutation (
        solution_id,
        data_id,
        mutation_index,
        key,
        value
    )
VALUES
    (
        (
            SELECT
                id
            FROM
                solution
            WHERE
                content_hash = :solution_hash
            LIMIT
                1
        ), 
        (
            SELECT
                solution_data.id
            FROM
                solution_data
                JOIN solution ON solution.id = solution_data.solution_id
            WHERE
                solution.content_hash = :solution_hash AND solution_data.data_index = :data_index
            LIMIT
                1
        ), :mutation_index, :key, :value
    );