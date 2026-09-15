ALTER TABLE "permission_requests" ADD COLUMN "is_used" INTEGER NOT NULL DEFAULT 0;

UPDATE "system"
SET "schema_migration_version" = '003',
    "applied_at" = datetime('now');
