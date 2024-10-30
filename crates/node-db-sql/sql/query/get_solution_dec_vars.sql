SELECT
    dec_var.dec_var_index,
    dec_var.value
FROM
    solution
    JOIN solution_data ON solution_data.solution_id = solution.id
    JOIN dec_var ON dec_var.solution_id = solution.id AND dec_var.data_id = solution_data.id
WHERE
    solution.content_hash = :content_hash AND solution_data.data_index = :data_index;
