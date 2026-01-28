ALTER TABLE appointments ADD COLUMN latitude REAL;
ALTER TABLE appointments ADD COLUMN longitude REAL;

CREATE TABLE IF NOT EXISTS push_subscriptions (
    id TEXT PRIMARY KEY,
    appointment_id TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (appointment_id) REFERENCES appointments(id) ON DELETE CASCADE,
    UNIQUE (appointment_id, endpoint)
);

CREATE INDEX IF NOT EXISTS idx_push_subscriptions_appointment ON push_subscriptions(appointment_id);
