-- Web Push subscriptions (Phase 3, section 9): one row per device that enabled notifications.
CREATE TABLE push_subscriptions (
    id         INTEGER PRIMARY KEY,
    endpoint   TEXT NOT NULL UNIQUE,    -- the push service URL; the key of the subscription
    p256dh     TEXT NOT NULL,           -- the browser's public key, base64url
    auth       TEXT NOT NULL,           -- the browser's auth secret, base64url
    label      TEXT,                    -- e.g. "iPhone", from the user agent
    created_at TEXT NOT NULL
);
