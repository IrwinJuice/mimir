-- Create bills table
CREATE TABLE IF NOT EXISTS bills
(
    id               TEXT PRIMARY KEY,
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

    FOREIGN KEY (ida) REFERENCES accounts (ida)
);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_bills_account_id ON bills (ida);
CREATE INDEX IF NOT EXISTS idx_bills_transaction_time ON bills (transaction_time);
CREATE INDEX IF NOT EXISTS idx_bills_id ON bills (id);
CREATE INDEX IF NOT EXISTS idx_bills_mcc_id ON bills (mcc);

