-- Create accounts table
CREATE TABLE IF NOT EXISTS bank_account
(
    ida        INTEGER PRIMARY KEY AUTOINCREMENT,
    idu        INTEGER     NOT NULL,
    kind       TEXT        NOT NULL,
    token      TEXT UNIQUE NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (idu) REFERENCES suser (idu) ON DELETE CASCADE
);

-- Create index for faster lookups
CREATE INDEX IF NOT EXISTS idx_bank_account_idu ON bank_account (idu);
CREATE INDEX IF NOT EXISTS idx_bank_account_kind ON bank_account (kind);

-- Create accounts_monitor table
CREATE TABLE IF NOT EXISTS bank_account_monitor
(
    ida             INTEGER,
    external_id     TEXT PRIMARY KEY,
    currency_code   INTEGER NOT NULL,
    balance         INTEGER NOT NULL,
    credit_limit    INTEGER NOT NULL,
    iban            TEXT    NOT NULL,
    masked_pan      TEXT    NOT NULL,
    kind            TEXT    NOT NULL,
    updated_at      DATETIME,
    last_taken_date DATETIME,
    FOREIGN KEY (ida) REFERENCES bank_account (ida) ON DELETE CASCADE
);

-- Create index on idu for faster lookups
CREATE INDEX IF NOT EXISTS idx_bank_account_monitor_external_id ON bank_account_monitor (external_id);