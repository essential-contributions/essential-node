SELECT
    mutation.key,
    mutation.value
FROM
    solution_set
    JOIN solution ON solution.solution_set_id = solution_set.id
    JOIN mutation ON mutation.solution_id = solution.id
WHERE
    solution_set.content_hash = :content_hash AND solution.solution_index = :solution_index;
ORDER BY
    mutation.mutation_index ASC
