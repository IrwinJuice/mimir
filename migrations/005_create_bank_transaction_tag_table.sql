-- Create tag table
CREATE TABLE IF NOT EXISTS bank_transaction_tag
(
    idt      TEXT,
    tag      TEXT,
    severity TEXT, -- primary, secondary, success, info, warn, danger, contrast

    FOREIGN KEY (idt) REFERENCES bank_transaction (id) ON DELETE CASCADE

);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_bank_transaction_tag_idt ON bank_transaction_tag (idt);
CREATE INDEX IF NOT EXISTS idx_bank_transaction_tag_tag ON bank_transaction_tag (tag);
