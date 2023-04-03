CREATE TABLE IF NOT EXISTS blocks (
    id          INTEGER      NOT NULL PRIMARY KEY AUTOINCREMENT,
    block_hash  TEXT         NOT NULL UNIQUE,
    height      TEXT         NOT NULL UNIQUE,
    block       BLOB         NOT NULL
);

CREATE TABLE IF NOT EXISTS block_certificates (
    id              INTEGER      NOT NULL PRIMARY KEY AUTOINCREMENT,
    block_hash        TEXT         NOT NULL UNIQUE,
    certificates    BLOB         NOT NULL UNIQUE
);