INSERT
    OR IGNORE INTO predicate (content_hash, predicate)
VALUES
    (:content_hash, :predicate);
