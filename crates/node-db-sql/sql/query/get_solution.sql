SELECT
    solution.contract_addr,
    solution.predicate_addr
FROM
    solution_set
    JOIN solution ON solution.solution_set_id = solution_set.id
WHERE
    solution_set.content_hash = ?
ORDER BY
    solution.solution_index ASC;
