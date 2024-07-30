SELECT
    contract_predicate.contract_id,
    predicate.predicate
FROM
    predicate
    JOIN contract_predicate ON predicate.id = contract_predicate.predicate_id
WHERE
    contract_predicate.contract_id IN (
        SELECT
            id
        FROM
            contract
        WHERE
            (
                created_at_seconds > :start_seconds
                OR (
                    created_at_seconds = :start_seconds
                    AND created_at_nanos >= :start_nanos
                )
            )
            AND (
                created_at_seconds < :end_seconds
                OR (
                    created_at_seconds = :end_seconds
                    AND created_at_nanos <= :end_nanos
                )
            )
        LIMIT
            :page_size OFFSET :page_size * :page_number
    )
ORDER BY
    contract_predicate.contract_id,
    contract_predicate.id;
