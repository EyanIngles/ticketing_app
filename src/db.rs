use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::time::Duration;

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect("tickets.db")
        .await?;

    sqlx::query!(
        r#"
        CREATE TABLE IF NOT EXISTS tickets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    println!("✅ Database initialized (tickets.db)");
    Ok(pool)
}
