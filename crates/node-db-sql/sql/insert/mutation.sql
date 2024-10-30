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
                id
            FROM
                solution_data
            WHERE
                data_index = :data_index
            LIMIT
                1
        ), :mutation_index, :key, :value
    );