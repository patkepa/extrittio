INSERT INTO device_types (tenant_id, name, icon, color_hex)
VALUES ('default', 'OrganBath', 'heatmap', '#E76A6E')
ON CONFLICT (tenant_id, name) DO NOTHING;
