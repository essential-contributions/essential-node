INSERT
    OR IGNORE INTO dec_var (
        solution_id,
        dec_var_index,
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
                solution_set.content_hash = :solution_set_hash AND solution.solution_index = :solution_index
            LIMIT
                1
        ), :dec_var_index, :value
    );
