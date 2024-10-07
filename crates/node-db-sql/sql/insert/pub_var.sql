INSERT
    OR IGNORE INTO pub_var (
        solution_id,
        data_index,
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
        ), :data_index, :key, :value
    );