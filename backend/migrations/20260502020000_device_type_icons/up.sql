ALTER TABLE device_types
    ADD COLUMN icon TEXT NOT NULL DEFAULT 'cube',
    ADD COLUMN color_hex TEXT NOT NULL DEFAULT '#8ABBFF';

ALTER TABLE device_types
    ADD CONSTRAINT device_types_icon_format
        CHECK (icon ~ '^[a-z0-9][a-z0-9_-]{0,63}$'),
    ADD CONSTRAINT device_types_color_hex_format
        CHECK (color_hex ~ '^#[0-9A-Fa-f]{6}$');

UPDATE device_types
SET icon = 'desktop',
    color_hex = '#F7C948'
WHERE name = 'mac-device';

UPDATE device_types
SET icon = 'antenna',
    color_hex = '#36CFC9'
WHERE name = 'network-analyzer';
