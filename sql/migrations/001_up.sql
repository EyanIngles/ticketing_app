ALTER TABLE users ADD COLUMN "username" TEXT;
ALTER TABLE users ADD COLUMN "type" TEXT;
ALTER TABLE users ADD COLUMN "role" TEXT;
ALTER TABLE users ADD COLUMN "name" TEXT;
ALTER TABLE users ADD COLUMN "default_model" TEXT;
ALTER TABLE users ADD COLUMN "token_hash" TEXT;
ALTER TABLE users ADD COLUMN "created_at" TEXT DEFAULT CURRENT_TIMESTAMP;
CREATE UNIQUE INDEX IF NOT EXISTS users_username_uq ON users ("username");

ALTER TABLE tickets ADD COLUMN "status" TEXT DEFAULT 'queued';
ALTER TABLE tickets ADD COLUMN "opencode_session_id" TEXT;
ALTER TABLE tickets ADD COLUMN "github_pr_url" TEXT;
ALTER TABLE tickets ADD COLUMN "last_model" TEXT;
ALTER TABLE tickets ADD COLUMN "updated_at" TEXT;

ALTER TABLE comments ADD COLUMN "author_id" INTEGER;
ALTER TABLE comments ADD COLUMN "author_name" TEXT;
ALTER TABLE comments ADD COLUMN "author_type" TEXT;
ALTER TABLE comments ADD COLUMN "author_role" TEXT;
ALTER TABLE comments ADD COLUMN "model" TEXT;
ALTER TABLE comments ADD COLUMN "format" TEXT DEFAULT 'markdown';
ALTER TABLE comments ADD COLUMN "created_at" TEXT DEFAULT CURRENT_TIMESTAMP;

ALTER TABLE system ADD COLUMN "schema_migration_version" TEXT;
ALTER TABLE system ADD COLUMN "applied_at" TEXT;

UPDATE system
SET "schema_migration_version" = '001',
    "applied_at" = datetime('now');

INSERT INTO system ("version", "date", "schema_migration_version", "applied_at")
SELECT '0.1.0', datetime('now'), '001', datetime('now')
WHERE NOT EXISTS (SELECT 1 FROM system);
