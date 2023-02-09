CREATE TABLE IF NOT EXISTS blocks (
    id        INTEGER      NOT NULL PRIMARY KEY AUTOINCREMENT,
    block_id  TEXT         NOT NULL UNIQUE,
    label     TEXT         UNIQUE,
    block     BLOB         NOT NULL
);