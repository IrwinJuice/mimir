-- Create tag table
CREATE UNIQUE INDEX IF NOT EXISTS uq_bank_account_ida ON bank_account (ida);
CREATE UNIQUE INDEX IF NOT EXISTS uq_bank_account_monitor_external_id ON bank_account_monitor (external_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_bank_transaction_idt ON bank_transaction (idt);

CREATE TABLE IF NOT EXISTS bank_transaction_tag
(
    idt      TEXT,
    tag      TEXT,
    severity TEXT, -- primary, secondary, success, info, warn, help, danger, contrast
    UNIQUE (idt, tag, severity)
);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_bank_transaction_tag_idt ON bank_transaction_tag (idt);
CREATE INDEX IF NOT EXISTS idx_bank_transaction_tag_tag ON bank_transaction_tag (tag);
