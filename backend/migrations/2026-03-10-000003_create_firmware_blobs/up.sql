CREATE TABLE firmware_blobs (
    firmware_update_id INTEGER PRIMARY KEY NOT NULL REFERENCES firmware_updates(id) ON DELETE CASCADE,
    data BLOB NOT NULL,
    size INTEGER NOT NULL,
    filename TEXT NOT NULL
);
