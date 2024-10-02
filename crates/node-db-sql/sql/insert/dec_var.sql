INSERT
    OR IGNORE INTO dec_var (
        solution_id,
        data_index,
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
        ), :data_index, :dec_var_index, :value
    );