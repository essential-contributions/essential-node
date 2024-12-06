INSERT
    OR IGNORE INTO pred_data (
        solution_id,
        pred_data_index,
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
        ), :pred_data_index, :value
    );
