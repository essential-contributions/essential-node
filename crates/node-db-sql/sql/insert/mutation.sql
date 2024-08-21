INSERT
    OR IGNORE INTO mutation (
        solution_id,
        data_index,
        mutation_index,
        contract_ca,
        KEY,
        value
    )
VALUES
    (
        (
            SELECT
                id
            FROM
                solution
            WHERE
                content_hash = :solution_hash
            LIMIT
                1
        ), :data_index, :mutation_index, :contract_ca, :key, :value
    );