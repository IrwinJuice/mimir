-- Create bills table
CREATE TABLE IF NOT EXISTS bills
(
    idb              INTEGER PRIMARY KEY AUTOINCREMENT,
    ida              INTEGER  NOT NULL,
    external_id      TEXT     NOT NULL,
    amount           INTEGER  NOT NULL,
    currency_code    INTEGER  NOT NULL,
    description      TEXT,
    mcc              INTEGER,
    hold             INTEGER,
    transaction_time DATETIME NOT NULL,
    created_at       DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (ida) REFERENCES accounts (ida)
);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_bills_account_id ON bills (ida);
CREATE INDEX IF NOT EXISTS idx_bills_transaction_time ON bills (transaction_time);
CREATE INDEX IF NOT EXISTS idx_bills_external_id ON bills (external_id);
CREATE INDEX IF NOT EXISTS idx_bills_mcc_id ON bills (mcc);

