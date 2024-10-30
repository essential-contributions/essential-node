SELECT
    solution_data.data_index,
    solution_data.contract_addr,
    solution_data.predicate_addr
FROM
    solution
    JOIN solution_data ON solution_data.solution_id = solution.id
WHERE
    solution.content_hash = ?;
