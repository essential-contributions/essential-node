INSERT
    OR IGNORE INTO dec_var (
        solution_id,
        data_id,
        dec_var_index,
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
        ), :dec_var_index, :value
    );