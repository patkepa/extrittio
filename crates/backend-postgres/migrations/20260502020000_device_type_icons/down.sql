ALTER TABLE device_types
    DROP CONSTRAINT IF EXISTS device_types_color_hex_format,
    DROP CONSTRAINT IF EXISTS device_types_icon_format;

ALTER TABLE device_types
    DROP COLUMN IF EXISTS color_hex,
    DROP COLUMN IF EXISTS icon;
