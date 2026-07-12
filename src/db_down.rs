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
        println!("Reverting database now... please wait..");
        sqlite::Sqlite::create_database(db_url).await?
    }
    let pool = sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await?;

    sqlx::query(
        "ALTER TABLE tickets DELETE COLUMN project_id;
        ",
    )
    .execute(&pool)
    .await?;

    print!("✅ successfully reverted database by deleting project_id column from tickets table.");
    Ok(pool)
}
