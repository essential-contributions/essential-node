INSERT
    OR IGNORE INTO dec_var (
        data_id,
        dec_var_index,
        value
    )
VALUES
    (
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
        ), :dec_var_index, :value
    );
