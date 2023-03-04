CREATE TABLE IF NOT EXISTS blocks (
    id        INTEGER      NOT NULL PRIMARY KEY AUTOINCREMENT,
    block_id  TEXT         NOT NULL UNIQUE,
    height    TEXT         NOT NULL UNIQUE,
    block     BLOB         NOT NULL
);

CREATE TABLE IF NOT EXISTS signatures (
    id          INTEGER      NOT NULL PRIMARY KEY AUTOINCREMENT,
    block_id    TEXT         NOT NULL UNIQUE,
    signatures  BLOB         NOT NULL UNIQUE
);