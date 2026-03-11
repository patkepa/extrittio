CREATE TABLE device_certificates (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    private_key_pem TEXT NOT NULL,
    certificate_pem TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
