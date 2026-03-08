-- Create bills table
CREATE TABLE IF NOT EXISTS bank_transaction
(
    id               TEXT PRIMARY KEY,
    external_id      TEXT,
    ida              INTEGER  NOT NULL,
    amount           INTEGER  NOT NULL,
    currency_code    INTEGER  NOT NULL,
    description      TEXT,
    mcc              INTEGER,
    hold             INTEGER,
    transaction_time DATETIME NOT NULL,
    created_at       DATETIME DEFAULT CURRENT_TIMESTAMP,
    receipt_id       TEXT,
    balance          INTEGER,

    FOREIGN KEY (ida) REFERENCES bank_account (ida) ON DELETE CASCADE,
    FOREIGN KEY (external_id) REFERENCES bank_account_monitor (external_id) ON DELETE CASCADE
    
);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_bank_transaction_account_id ON bank_transaction (ida);
CREATE INDEX IF NOT EXISTS idx_bank_transaction_transaction_time ON bank_transaction (transaction_time);
CREATE INDEX IF NOT EXISTS idx_bank_transaction_id ON bank_transaction (id);
CREATE INDEX IF NOT EXISTS idx_bank_transaction_mcc_id ON bank_transaction (mcc);

