use crate::auth::hash_password;
use sqlx::SqlitePool;

pub async fn seed(pool: &SqlitePool) -> Result<(), String> {
    if let Some(human) = human_from_env() {
        upsert_human(pool, &human).await?;
    } else {
        println!("seed: skipping human (LYRA_HUMAN_USERNAME / LYRA_HUMAN_PASSWORD not set)");
    }

    if let Some(agent) = agent_from_env() {
        upsert_agent(pool, &agent).await?;
    } else {
        println!("seed: skipping agent (LYRA_AGENT_USER / LYRA_AGENT_TOKEN not set)");
    }

    if let Ok(client_id) = dotenv::var("OAUTH_CLIENT_ID") {
        let client_id = client_id.trim().to_string();
        if !client_id.is_empty() {
            let name = dotenv::var("OAUTH_CLIENT_NAME").unwrap_or_else(|_| "Lyra iOS".into());
            upsert_oauth_client(pool, &client_id, &name).await?;
        }
    } else {
        println!("seed: skipping oauth client (OAUTH_CLIENT_ID not set)");
    }

    Ok(())
}

struct HumanSeed {
    username: String,
    password: String,
    name: String,
    role: String,
    email: String,
}

struct AgentSeed {
    username: String,
    token: String,
    name: String,
    role: String,
    default_model: String,
}

fn human_from_env() -> Option<HumanSeed> {
    let username = nonempty_env("LYRA_HUMAN_USERNAME")?;
    let password = nonempty_env("LYRA_HUMAN_PASSWORD")?;
    Some(HumanSeed {
        username,
        password,
        name: nonempty_env("LYRA_HUMAN_NAME").unwrap_or_else(|| "Eyan".into()),
        role: nonempty_env("LYRA_HUMAN_ROLE").unwrap_or_else(|| "Owner".into()),
        email: nonempty_env("LYRA_HUMAN_EMAIL").unwrap_or_else(|| "local@lyra".into()),
    })
}

fn agent_from_env() -> Option<AgentSeed> {
    let username = nonempty_env("LYRA_AGENT_USER")?;
    let token = nonempty_env("LYRA_AGENT_TOKEN")?;
    Some(AgentSeed {
        username,
        token,
        name: nonempty_env("LYRA_AGENT_NAME").unwrap_or_else(|| "Mark".into()),
        role: nonempty_env("LYRA_AGENT_ROLE").unwrap_or_else(|| "Engineer".into()),
        default_model: nonempty_env("LYRA_AGENT_DEFAULT_MODEL").unwrap_or_else(|| "Grok4.6".into()),
    })
}

fn nonempty_env(key: &str) -> Option<String> {
    dotenv::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

async fn username_exists(pool: &SqlitePool, username: &str) -> Result<bool, String> {
    let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM users WHERE "username" = $1"#)
        .bind(username)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

async fn upsert_human(pool: &SqlitePool, human: &HumanSeed) -> Result<(), String> {
    if username_exists(pool, &human.username).await? {
        println!("seed: human {} already present", human.username);
        return Ok(());
    }
    let password_hash = hash_password(&human.password)?;
    sqlx::query(
        r#"INSERT INTO users ("email", "password", "username", "type", "role", "name")
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(&human.email)
    .bind(password_hash)
    .bind(&human.username)
    .bind("Human")
    .bind(&human.role)
    .bind(&human.name)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    println!("seed: created human {}", human.username);
    Ok(())
}

async fn upsert_agent(pool: &SqlitePool, agent: &AgentSeed) -> Result<(), String> {
    if username_exists(pool, &agent.username).await? {
        println!("seed: agent {} already present", agent.username);
        return Ok(());
    }
    let token_hash = hash_password(&agent.token)?;
    sqlx::query(
        r#"INSERT INTO users ("email", "password", "username", "type", "role", "name", "default_model", "token_hash")
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(format!("{}@agent.lyra", agent.username))
    .bind(&token_hash)
    .bind(&agent.username)
    .bind("Agent")
    .bind(&agent.role)
    .bind(&agent.name)
    .bind(&agent.default_model)
    .bind(&token_hash)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    println!("seed: created agent {}", agent.username);
    Ok(())
}

async fn upsert_oauth_client(pool: &SqlitePool, client_id: &str, name: &str) -> Result<(), String> {
    let row = sqlx::query(r#"SELECT "id" FROM oauth_clients WHERE "client_id" = $1"#)
        .bind(client_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    if row.is_some() {
        println!("seed: oauth client {client_id} already present");
        return Ok(());
    }
    sqlx::query(r#"INSERT INTO oauth_clients ("client_id", "name") VALUES ($1, $2)"#)
        .bind(client_id)
        .bind(name)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    println!("seed: created oauth client {client_id}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            r#"
            CREATE TABLE comments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ticket_id INTEGER NOT NULL,
                text TEXT NOT NULL
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
        .execute(&pool)
        .await
        .unwrap();
        run_migrations(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn seeds_human_agent_and_client_once() {
        let pool = setup_pool().await;
        let human = HumanSeed {
            username: "eyan".into(),
            password: "secret-pass".into(),
            name: "Eyan".into(),
            role: "Owner".into(),
            email: "eyan@local".into(),
        };
        let agent = AgentSeed {
            username: "mark-eng".into(),
            token: "agent-token".into(),
            name: "Mark".into(),
            role: "Engineer".into(),
            default_model: "Grok4.6".into(),
        };

        upsert_human(&pool, &human).await.unwrap();
        upsert_human(&pool, &human).await.unwrap();
        upsert_agent(&pool, &agent).await.unwrap();
        upsert_agent(&pool, &agent).await.unwrap();
        upsert_oauth_client(&pool, "lyra-ios", "Lyra iOS")
            .await
            .unwrap();
        upsert_oauth_client(&pool, "lyra-ios", "Lyra iOS")
            .await
            .unwrap();

        let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(users, 2);

        let kind: String =
            sqlx::query_scalar(r#"SELECT "type" FROM users WHERE "username" = 'mark-eng'"#)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(kind, "Agent");

        let clients: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_clients")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(clients, 1);
    }
}
