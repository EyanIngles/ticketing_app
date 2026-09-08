ALTER TABLE system DROP COLUMN "applied_at";
ALTER TABLE system DROP COLUMN "schema_migration_version";

ALTER TABLE comments DROP COLUMN "created_at";
ALTER TABLE comments DROP COLUMN "format";
ALTER TABLE comments DROP COLUMN "model";
ALTER TABLE comments DROP COLUMN "author_role";
ALTER TABLE comments DROP COLUMN "author_type";
ALTER TABLE comments DROP COLUMN "author_name";
ALTER TABLE comments DROP COLUMN "author_id";

ALTER TABLE tickets DROP COLUMN "updated_at";
ALTER TABLE tickets DROP COLUMN "last_model";
ALTER TABLE tickets DROP COLUMN "github_pr_url";
ALTER TABLE tickets DROP COLUMN "opencode_session_id";
ALTER TABLE tickets DROP COLUMN "status";

DROP INDEX IF EXISTS users_username_uq;
ALTER TABLE users DROP COLUMN "created_at";
ALTER TABLE users DROP COLUMN "token_hash";
ALTER TABLE users DROP COLUMN "default_model";
ALTER TABLE users DROP COLUMN "name";
ALTER TABLE users DROP COLUMN "role";
ALTER TABLE users DROP COLUMN "type";
ALTER TABLE users DROP COLUMN "username";
