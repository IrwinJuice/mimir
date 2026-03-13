ALTER TABLE bank_transaction RENAME COLUMN id TO idt;
CREATE INDEX IF NOT EXISTS idx_bank_transaction_idt ON bank_transaction (idt);
CREATE INDEX IF NOT EXISTS idx_bank_transaction_account_id ON bank_transaction (ida);
CREATE INDEX IF NOT EXISTS idx_bank_transaction_transaction_time ON bank_transaction (transaction_time);
CREATE INDEX IF NOT EXISTS idx_bank_transaction_mcc_id ON bank_transaction (mcc);
