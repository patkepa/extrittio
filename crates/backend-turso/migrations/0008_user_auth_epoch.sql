ALTER TABLE users ADD COLUMN auth_epoch TEXT;

UPDATE users
SET auth_epoch = lower(hex(randomblob(16)))
WHERE auth_epoch IS NULL;

CREATE UNIQUE INDEX users_auth_epoch_key ON users (auth_epoch);

CREATE TRIGGER users_auth_epoch_required_insert
BEFORE INSERT ON users
FOR EACH ROW
WHEN NEW.auth_epoch IS NULL OR NEW.auth_epoch = ''
BEGIN
  SELECT RAISE(ABORT, 'users.auth_epoch is required');
END;

CREATE TRIGGER users_auth_epoch_required_update
BEFORE UPDATE OF auth_epoch ON users
FOR EACH ROW
WHEN NEW.auth_epoch IS NULL OR NEW.auth_epoch = ''
BEGIN
  SELECT RAISE(ABORT, 'users.auth_epoch is required');
END;
