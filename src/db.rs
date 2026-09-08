use crate::migrate::run_migrations;
use sqlx::{SqlitePool, migrate::MigrateDatabase, sqlite};

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let db_url = "sqlite:tickets.db";

    if !sqlite::Sqlite::database_exists(db_url)
        .await
        .unwrap_or(false)
    {
        println!("Creating new database: tickets.db");
        sqlite::Sqlite::create_database(db_url).await?;
    } else {
        println!("Database already exists");
    }

    let pool = sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await?;

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    run_migrations(&pool).await?;

    println!("Database initialized successfully (tickets.db)");
    Ok(pool)
}
