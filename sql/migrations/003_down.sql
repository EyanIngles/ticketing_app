ALTER TABLE "permission_requests" DROP COLUMN "is_used";

UPDATE "system"
SET "schema_migration_version" = '002',
    "applied_at" = datetime('now');
