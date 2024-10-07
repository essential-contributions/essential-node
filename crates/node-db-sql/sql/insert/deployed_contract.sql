INSERT
    OR IGNORE INTO deployed_contract (contract_hash, mutation_id)
VALUES
    (
        :contract_hash,
        (
            SELECT
                mutation.id
            FROM
                mutation
            WHERE
                mutation.solution_id = (
                    SELECT 
                        id 
                    FROM 
                        solution 
                    WHERE 
                        solution.content_hash = :solution_hash LIMIT 1
                ) 
                AND mutation.data_index = :mutation_data_index
                AND mutation.mutation_index = :mutation_index
            LIMIT
                1
        )
    );