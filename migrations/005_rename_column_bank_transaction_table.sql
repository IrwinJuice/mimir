-- rename column "id" to "idt" in bank_transaction;
DROP INDEX idx_bank_transaction_id;
ALTER TABLE bank_transaction RENAME COLUMN id TO idt;
CREATE INDEX IF NOT EXISTS idx_bank_transaction_idt ON bank_transaction (idt);