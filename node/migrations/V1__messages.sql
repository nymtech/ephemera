CREATE TABLE IF NOT EXISTS messages
(
    id                  INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    created_at          DATETIME      NOT NULL,
    request_id          TEXT         NOT NULL,
    message             BLOB         NOT NULL,
    signatures          BLOB         NOT NULL
);