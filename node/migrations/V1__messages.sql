CREATE TABLE IF NOT EXISTS messages
(
    id                  INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    created_at          DATETIME     NOT NULL,
    request_id          TEXT         NOT NULL,
    custom_message_id   TEXT         NOT NULL UNIQUE,
    message             BLOB         NOT NULL,
    signatures          BLOB         NOT NULL
);