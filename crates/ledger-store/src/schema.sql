-- The ledger's journal. Applied on every open; every statement is idempotent.

-- Every event the ledger has recorded, in order. Rows are never updated or
-- deleted: the journal is the history, and state is rebuilt from it.
CREATE TABLE IF NOT EXISTS events (
    seq  INTEGER PRIMARY KEY AUTOINCREMENT,
    body TEXT NOT NULL
);

-- One row per job attempt that started an effect. `intent_at` is written
-- before the effect runs; `outcome` is written after it finishes. A row with
-- no outcome is an effect whose fate is unknown (ADR 0003).
CREATE TABLE IF NOT EXISTS effects (
    job       INTEGER NOT NULL,
    attempt   INTEGER NOT NULL,
    intent_at INTEGER NOT NULL,
    outcome   TEXT,
    PRIMARY KEY (job, attempt)
);
