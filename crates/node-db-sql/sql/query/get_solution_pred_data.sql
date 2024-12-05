SELECT
    pred_data.value
FROM
    solution_set
    JOIN solution ON solution.solution_set_id = solution_set.id
    JOIN pred_data ON pred_data.solution_id = solution.id
WHERE
    solution_set.content_hash = :content_hash AND solution.solution_index = :solution_index;
ORDER BY
    pred_data.pred_data_index ASC
