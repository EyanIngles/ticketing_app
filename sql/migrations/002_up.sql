CREATE TABLE IF NOT EXISTS "permission_requests" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "ticket_id" INTEGER,
    "author_id" INTEGER,
    "author_name" TEXT,
    "author_type" TEXT,
    "author_role" TEXT,
    "model" TEXT,
    "opencode_session_id" TEXT,
    "permission_id" TEXT,
    "tool" TEXT,
    "payload" TEXT,
    "status" TEXT DEFAULT 'pending',
    "created_at" TIMESTAMP DEFAULT (datetime('now')),
    FOREIGN KEY ("ticket_id") REFERENCES "tickets" ("id")
);

CREATE TABLE IF NOT EXISTS "oauth_clients" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "client_id" TEXT NOT NULL UNIQUE,
    "name" TEXT,
    "created_at" TIMESTAMP DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS "oauth_codes" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "client_id" TEXT,
    "code" TEXT NOT NULL UNIQUE,
    "code_challenge" TEXT,
    "username" TEXT,
    "expires_at" TEXT,
    "created_at" TIMESTAMP DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS "refresh_tokens" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "token_hash" TEXT NOT NULL UNIQUE,
    "user_id" INTEGER,
    "expires_at" TEXT,
    "created_at" TIMESTAMP DEFAULT (datetime('now')),
    FOREIGN KEY ("user_id") REFERENCES "users" ("id")
);

CREATE TABLE IF NOT EXISTS "request_logs" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "request_id" TEXT,
    "method" TEXT,
    "path" TEXT,
    "status" INTEGER,
    "duration_ms" INTEGER,
    "user_id" INTEGER,
    "user_name" TEXT,
    "created_at" TIMESTAMP DEFAULT (datetime('now'))
);

UPDATE "system"
SET "schema_migration_version" = '002',
    "applied_at" = datetime('now');
