DROP TABLE IF EXISTS device_blueprint_revisions;
DROP TABLE IF EXISTS device_blueprint_drafts;
DROP TABLE IF EXISTS device_blueprints;
DELETE FROM role_permissions
WHERE permission IN ('device_blueprints.read', 'device_blueprints.manage');
