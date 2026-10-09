-- TELEVAULT Telegram Authentication and Channel Metadata
-- Version 6: Telegram account profile cache and backup channel settings (no secrets stored)

CREATE TABLE IF NOT EXISTS telegram_accounts (
    user_id INTEGER PRIMARY KEY NOT NULL,
    first_name TEXT NOT NULL,
    last_name TEXT,
    username TEXT,
    phone_redacted TEXT NOT NULL,
    authenticated_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS telegram_channels (
    channel_id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL REFERENCES telegram_accounts(user_id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    is_private INTEGER NOT NULL DEFAULT 1,
    verified INTEGER NOT NULL DEFAULT 0,
    created_by_televault INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    verified_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_telegram_channels_user_id ON telegram_channels(user_id);

CREATE TABLE IF NOT EXISTS telegram_auth_state (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    current_state TEXT NOT NULL,
    active_user_id INTEGER REFERENCES telegram_accounts(user_id) ON DELETE SET NULL,
    active_channel_id INTEGER REFERENCES telegram_channels(channel_id) ON DELETE SET NULL,
    updated_at TEXT NOT NULL
);
