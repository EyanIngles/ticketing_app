DROP TABLE IF EXISTS "request_logs";
DROP TABLE IF EXISTS "refresh_tokens";
DROP TABLE IF EXISTS "oauth_codes";
DROP TABLE IF EXISTS "oauth_clients";
DROP TABLE IF EXISTS "permission_requests";

UPDATE "system"
SET "schema_migration_version" = '001',
    "applied_at" = datetime('now');
