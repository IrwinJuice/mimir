-- Migration 007
-- Rename columns in `bank_account_monitor` using ALTER TABLE ... RENAME COLUMN
-- Requires SQLite >= 3.25.0 (2018-09-15)
--   - `updated_at`      -> `range_end`
--   - `last_taken_date` -> `range_start`

-- Rename columns
ALTER TABLE bank_account_monitor RENAME COLUMN updated_at TO range_end;
ALTER TABLE bank_account_monitor RENAME COLUMN last_taken_date TO range_start;

