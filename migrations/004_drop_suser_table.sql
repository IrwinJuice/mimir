-- Migration 004: remove `idu` column and the foreign key to `suser` from `bank_account`.
-- Strategy: clone dependent tables before dropping anything, then restore data to avoid cascades wiping rows.

PRAGMA foreign_keys = OFF;

-- Backup tables
CREATE TABLE IF NOT EXISTS bank_account_monitor_backup AS SELECT * FROM bank_account_monitor;
CREATE TABLE IF NOT EXISTS bank_transaction_backup AS SELECT * FROM bank_transaction;

-- Drop index on idu if it exists
DROP INDEX IF EXISTS idx_bank_account_idu;

-- Create new table without `idu` (and without the foreign key to suser)
CREATE TABLE IF NOT EXISTS bank_account_new
(
    ida        INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT        NOT NULL,
    token      TEXT UNIQUE NOT NULL,
    name       TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Copy data from old table. We intentionally omit `idu` here (it will be removed).
INSERT INTO bank_account_new (ida, kind, token, created_at, updated_at, name)
SELECT ida, kind, token, created_at, updated_at, kind || cast(ida as TEXT)
FROM bank_account;

-- Drop old table and rename the new one
DROP TABLE bank_account;
ALTER TABLE bank_account_new RENAME TO bank_account;

-- Recreate indexes that are still relevant
CREATE INDEX IF NOT EXISTS idx_bank_account_kind ON bank_account (kind);

-- Restore dependent tables from backups
DROP TABLE IF EXISTS bank_account_monitor;
CREATE TABLE bank_account_monitor AS SELECT * FROM bank_account_monitor_backup;
DROP TABLE bank_transaction;
CREATE TABLE bank_transaction AS SELECT * FROM bank_transaction_backup;

-- Clean up backups
DROP TABLE IF EXISTS bank_account_monitor_backup;
DROP TABLE IF EXISTS bank_transaction_backup;

DROP TABLE IF EXISTS suser;

PRAGMA foreign_keys = ON;

