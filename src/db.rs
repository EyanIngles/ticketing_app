use sqlx::{SqlitePool, migrate::MigrateDatabase, sqlite};

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let db_url = "sqlite:tickets.db";

    // Create the database file if it doesn't exist
    if !sqlite::Sqlite::database_exists(db_url)
        .await
        .unwrap_or(false)
    {
        println!("🛠 Creating new database: tickets.db");
        sqlite::Sqlite::create_database(db_url).await?;
    } else {
        println!("✅ Database already exists");
    }

    // Connect to the database
    let pool = sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await?;

    // Create tables (safe to run multiple times)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL UNIQUE,
            password TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT
        );

        CREATE TABLE IF NOT EXISTS tickets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );

        CREATE TABLE IF NOT EXISTS comments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ticket_id INTEGER NOT NULL,
            text TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (ticket_id) REFERENCES tickets(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    println!("✅ Database initialized successfully (tickets.db)");
    Ok(pool)
}
