SELECT
    predicate.predicate
FROM
    predicate
WHERE
    predicate.content_hash = :predicate_hash;
