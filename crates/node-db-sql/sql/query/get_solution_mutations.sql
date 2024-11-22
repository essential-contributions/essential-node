SELECT
    mutation.key,
    mutation.value
FROM
    solution
    JOIN solution_data ON solution_data.solution_id = solution.id
    JOIN mutation ON mutation.data_id = solution_data.id
WHERE
    solution.content_hash = :content_hash AND solution_data.data_index = :data_index;
ORDER BY
    mutation.mutation_index ASC