ALTER TABLE users
    ADD CONSTRAINT users_auth_epoch_nonempty CHECK (auth_epoch <> '');
