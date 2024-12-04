SELECT
    dec_var.value
FROM
    solution_set
    JOIN solution ON solution.solution_set_id = solution_set.id
    JOIN dec_var ON dec_var.solution_id = solution.id
WHERE
    solution_set.content_hash = :content_hash AND solution.solution_index = :solution_index;
ORDER BY
    dec_var.dec_var_index ASC
