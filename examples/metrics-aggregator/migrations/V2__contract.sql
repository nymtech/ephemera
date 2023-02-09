CREATE TABLE contract_mixnode_reward
(
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    mix_id             INTEGER NOT NULL,
    epoch              INTEGER NOT NULL,
    reliability        INTEGER NOT NULL,
    timestamp          INTEGER NOT NULL,
    UNIQUE (mix_id, epoch)
);