use sqlx::{
    ConnectOptions, Connection, SqlitePool,
    migrate::MigrateDatabase,
    query,
    sqlite::{self, SqliteConnectOptions},
};
use std::time::Duration;

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let DB_URL = "sqlite:tickets.db";
    if !sqlite::Sqlite::database_exists(DB_URL)
        .await
        .unwrap_or(false)
    {
        println!("Creating database now... please wait..");
        sqlite::Sqlite::create_database(DB_URL).await?
    }
    let pool = sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(DB_URL)
        .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS tickets (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP
);"#,
    )
    .execute(&pool)
    .await?;

    println!("✅ Database initialized (tickets.db)");
    Ok(pool)
}

