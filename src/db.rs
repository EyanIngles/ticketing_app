use sqlx::{
    SqlitePool,
    migrate::MigrateDatabase,
    sqlite::{self},
};

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let db_url = "sqlite:tickets.db";
    if !sqlite::Sqlite::database_exists(db_url)
        .await
        .unwrap_or(false)
    {
        println!("Creating database now... please wait..");
        sqlite::Sqlite::create_database(db_url).await?
    }
    let pool = sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
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
