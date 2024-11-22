WITH RECURSIVE chain AS (
    -- Base case: start with the given block address
    -- Note the key may be found here
    SELECT
        b.id AS block_id,
        b.parent_block_id,
        b.block_address,
        b.number AS number,
        m.value AS found_value,
        0 AS depth
    FROM
        block b
        LEFT JOIN block_solution bs ON bs.block_id = b.id
        LEFT JOIN solution_data ON solution_data.solution_id = bs.solution_id AND solution_data.contract_addr = :contract_ca
        LEFT JOIN mutation m ON m.data_id = solution_data.id
        AND m.key = :key
    WHERE
        b.block_address = :block_address
    UNION
    ALL -- Recursive case: follow parent pointers until we either find a value or hit a finalized block
    SELECT
        b.id AS block_id,
        b.parent_block_id,
        b.block_address,
        b.number AS number,
        m.value AS found_value,
        c.depth + 1
    FROM
        chain c
        JOIN block b ON b.id = c.parent_block_id
        LEFT JOIN solution_data ON solution_data.solution_id = b.id AND solution_data.contract_addr = :contract_ca
        LEFT JOIN mutation m ON m.data_id = solution_data.id
        AND m.key = :key
    WHERE
        c.found_value IS NULL -- Stop recursing if we found a value
        AND NOT EXISTS (
            -- Stop recursing if we hit a finalized block
            SELECT
                1
            FROM
                finalized_block fb
            WHERE
                fb.block_id = c.block_id
        )
)
SELECT
    (
        -- The key was found in this block but we
        -- have to find the latest version within this block
        SELECT
            value
        FROM
            block_solution bs
            JOIN solution_data ON solution_data.solution_id = bs.solution_id AND solution_data.contract_addr = :contract_ca
            JOIN mutation m ON m.data_id = solution_data.id
        WHERE
            bs.block_id = chain.block_id
            AND m.key = :key
            AND (
                :solution_index IS NULL
                OR bs.solution_index <= :solution_index
            )
        ORDER BY
            bs.solution_index DESC
    ) AS found_value,
    number
FROM
    chain
WHERE
    found_value IS NOT NULL
ORDER BY
    depth ASC
LIMIT
    1;