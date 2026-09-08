use sqlx::SqlitePool;

const MIGRATIONS: &[(&str, &str, &str)] = &[
    (
        "001",
        include_str!("../sql/migrations/001_up.sql"),
        include_str!("../sql/migrations/001_down.sql"),
    ),
    (
        "002",
        include_str!("../sql/migrations/002_up.sql"),
        include_str!("../sql/migrations/002_down.sql"),
    ),
];

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let applied = current_schema_migration_version(pool).await;

    for (version, up_sql, down_sql) in MIGRATIONS {
        if applied.as_deref() >= Some(*version) {
            println!("migration {version} already applied");
            continue;
        }

        println!("applying migration {version}…");
        match exec_sql(pool, up_sql, true).await {
            Ok(()) => {
                println!("migration {version} applied");
            }
            Err(err) => {
                eprintln!("migration {version} failed: {err}");
                eprintln!("rolling back {version}…");
                if let Err(down_err) = exec_sql(pool, down_sql, true).await {
                    eprintln!("rollback {version} failed: {down_err}");
                } else {
                    eprintln!("migration {version} rolled back");
                }
                return Err(err);
            }
        }
    }

    Ok(())
}

async fn current_schema_migration_version(pool: &SqlitePool) -> Option<String> {
    sqlx::query_scalar("SELECT schema_migration_version FROM system LIMIT 1")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

fn is_skippable_schema_error(err: &sqlx::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("duplicate column") || msg.contains("no such column")
}

async fn exec_sql(
    pool: &SqlitePool,
    sql: &str,
    skip_schema_conflicts: bool,
) -> Result<(), sqlx::Error> {
    for statement in split_sql(sql) {
        if let Err(err) = sqlx::query(&statement).execute(pool).await {
            if skip_schema_conflicts && is_skippable_schema_error(&err) {
                println!("skipping: {err}");
                continue;
            }
            return Err(err);
        }
    }
    Ok(())
}

fn split_sql(sql: &str) -> Vec<String> {
    sql.split(';')
        .map(|chunk| {
            chunk
                .lines()
                .filter(|line| !line.trim().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn seed_live_schema(pool: &SqlitePool) {
        exec_sql(
            pool,
            r#"
            CREATE TABLE comments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ticket_id INTEGER NOT NULL,
                text TEXT NOT NULL,
                FOREIGN KEY (ticket_id) REFERENCES tickets(id)
            );
            CREATE TABLE projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT
            );
            CREATE TABLE system (version TEXT NOT NULL, date TEXT NOT NULL);
            CREATE TABLE tickets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                project_id INTEGER REFERENCES projects(id)
            );
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL,
                password TEXT NOT NULL
            );
            INSERT INTO system (version, date) VALUES ('0.1.0', '12 July 2026');
            "#,
            false,
        )
        .await
        .unwrap();
    }

    async fn assert_schema_version(pool: &SqlitePool, expected: &str) {
        let version: String = sqlx::query_scalar("SELECT schema_migration_version FROM system")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(version, expected);
    }

    #[tokio::test]
    async fn applies_001_and_is_idempotent() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed_live_schema(&pool).await;

        run_migrations(&pool).await.unwrap();
        run_migrations(&pool).await.unwrap();

        assert_schema_version(&pool, "002").await;

        let status_cols: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('tickets') WHERE name = 'status'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status_cols, 1);
    }

    #[tokio::test]
    async fn finishes_after_partial_users_columns() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed_live_schema(&pool).await;

        exec_sql(
            &pool,
            r#"
            ALTER TABLE users ADD COLUMN "username" TEXT;
            ALTER TABLE users ADD COLUMN "type" TEXT;
            ALTER TABLE users ADD COLUMN "role" TEXT;
            ALTER TABLE users ADD COLUMN "name" TEXT;
            ALTER TABLE users ADD COLUMN "default_model" TEXT;
            ALTER TABLE users ADD COLUMN "token_hash" TEXT;
            "#,
            false,
        )
        .await
        .unwrap();

        run_migrations(&pool).await.unwrap();

        assert_schema_version(&pool, "002").await;

        let created_at: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'created_at'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(created_at, 1);
    }

    #[tokio::test]
    async fn down_drops_system_last_after_partial_up() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed_live_schema(&pool).await;

        exec_sql(
            &pool,
            r#"
            ALTER TABLE system ADD COLUMN "schema_migration_version" TEXT;
            ALTER TABLE system ADD COLUMN "applied_at" TEXT;
            ALTER TABLE users ADD COLUMN "username" TEXT;
            "#,
            false,
        )
        .await
        .unwrap();

        exec_sql(&pool, include_str!("../sql/migrations/001_down.sql"), true)
            .await
            .unwrap();

        let username: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'username'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(username, 0);

        let applied_at: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('system') WHERE name = 'applied_at'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(applied_at, 0);
    }

    #[tokio::test]
    async fn failed_up_runs_down_without_applied_at() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed_live_schema(&pool).await;

        exec_sql(
            &pool,
            r#"
            ALTER TABLE users ADD COLUMN "username" TEXT;
            ALTER TABLE users ADD COLUMN "token_hash" TEXT;
            "#,
            false,
        )
        .await
        .unwrap();

        exec_sql(&pool, include_str!("../sql/migrations/001_down.sql"), true)
            .await
            .unwrap();

        let username: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'username'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(username, 0);

        let applied_at: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('system') WHERE name = 'applied_at'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(applied_at, 0);
    }

    #[tokio::test]
    async fn applies_002_and_down_restores_001() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed_live_schema(&pool).await;

        run_migrations(&pool).await.unwrap();

        assert_schema_version(&pool, "002").await;

        let tables: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('permission_requests', 'oauth_clients', 'oauth_codes', 'refresh_tokens', 'request_logs')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tables, 5);

        exec_sql(&pool, include_str!("../sql/migrations/002_down.sql"), true)
            .await
            .unwrap();

        assert_schema_version(&pool, "001").await;

        let tables: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('permission_requests', 'oauth_clients', 'oauth_codes', 'refresh_tokens', 'request_logs')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tables, 0);
    }
}
