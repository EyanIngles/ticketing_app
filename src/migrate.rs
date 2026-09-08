use sqlx::SqlitePool;

const MIGRATIONS: &[(&str, &str, &str)] = &[(
    "001",
    include_str!("../sql/migrations/001_up.sql"),
    include_str!("../sql/migrations/001_down.sql"),
)];

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let applied = current_schema_migration_version(pool).await;

    for (version, up_sql, down_sql) in MIGRATIONS {
        if applied.as_deref() >= Some(*version) {
            println!("migration {version} already applied");
            continue;
        }

        println!("applying migration {version}…");
        match exec_sql(pool, up_sql).await {
            Ok(()) => {
                println!("migration {version} applied");
            }
            Err(err) => {
                eprintln!("migration {version} failed: {err}");
                eprintln!("rolling back {version}…");
                if let Err(down_err) = exec_sql(pool, down_sql).await {
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

async fn exec_sql(pool: &SqlitePool, sql: &str) -> Result<(), sqlx::Error> {
    for statement in split_sql(sql) {
        sqlx::query(&statement).execute(pool).await?;
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
        )
        .await
        .unwrap();
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

        let version: String = sqlx::query_scalar("SELECT schema_migration_version FROM system")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(version, "001");

        let status_cols: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('tickets') WHERE name = 'status'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status_cols, 1);
    }

    #[tokio::test]
    async fn failed_up_runs_down() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed_live_schema(&pool).await;

        let up = "ALTER TABLE tickets ADD COLUMN status TEXT DEFAULT 'queued'; SELECT * FROM missing_table;";
        let down = "ALTER TABLE tickets DROP COLUMN status;";

        assert!(exec_sql(&pool, up).await.is_err());
        exec_sql(&pool, down).await.unwrap();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('tickets') WHERE name = 'status'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
    }
}
