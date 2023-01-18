CREATE TABLE IF NOT EXISTS signatures
(
    id                  INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    created_at          DATETIME      NOT NULL,
    request_id          TEXT         NOT NULL,
    payload             BLOB         NOT NULL,
    signatures          BLOB         NOT NULL
);