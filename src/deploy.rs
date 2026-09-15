use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub async fn run_deploy(pool: &SqlitePool, ticket_id: i64) {
    let Some(repo_dir) = env_nonempty("LYRA_REPO_DIR") else {
        let _ = insert_system_comment(
            pool,
            ticket_id,
            "Deploy skipped: LYRA_REPO_DIR is not set.",
        )
        .await;
        return;
    };

    let mut snapshot: Option<Snapshot> = None;
    match deploy_live(pool, &repo_dir, &mut snapshot).await {
        Ok(git_ref) => {
            let _ = insert_system_comment(
                pool,
                ticket_id,
                &format!("Deploy succeeded (`{git_ref}`)."),
            )
            .await;
            let _ = set_status(pool, ticket_id, "awaiting_you").await;
            if let Err(err) = maybe_restart().await {
                revert_and_fail(pool, ticket_id, snapshot.as_ref(), &err).await;
            }
        }
        Err(err) => {
            revert_and_fail(pool, ticket_id, snapshot.as_ref(), &err).await;
        }
    }
}

struct Snapshot {
    bin_path: String,
    bak_path: PathBuf,
}

fn env_nonempty(key: &str) -> Option<String> {
    dotenv::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn bak_path(bin_path: &str) -> PathBuf {
    PathBuf::from(format!("{bin_path}.bak"))
}

fn snapshot_binary(bin_path: &str) -> Result<Option<Snapshot>, String> {
    let src = Path::new(bin_path);
    if !src.is_file() {
        return Ok(None);
    }
    let dest = bak_path(bin_path);
    std::fs::copy(src, &dest).map_err(|e| format!("snapshot binary failed: {e}"))?;
    Ok(Some(Snapshot {
        bin_path: bin_path.to_string(),
        bak_path: dest,
    }))
}

fn restore_binary(snapshot: &Snapshot) -> Result<(), String> {
    std::fs::copy(&snapshot.bak_path, &snapshot.bin_path)
        .map_err(|e| format!("restore binary failed: {e}"))?;
    Ok(())
}

fn install_release_binary(repo_dir: &str, bin_path: &str) -> Result<(), String> {
    let src = Path::new(repo_dir).join("target/release/ticketing-app-1");
    if !src.is_file() {
        return Err("release binary missing: target/release/ticketing-app-1".into());
    }
    std::fs::copy(&src, bin_path).map_err(|e| format!("install binary failed: {e}"))?;
    Ok(())
}

async fn deploy_live(
    pool: &SqlitePool,
    repo_dir: &str,
    snapshot: &mut Option<Snapshot>,
) -> Result<String, String> {
    let remote = env_nonempty("LYRA_DEPLOY_GIT_REMOTE").unwrap_or_else(|| "origin".into());
    let git_ref = env_nonempty("LYRA_DEPLOY_GIT_REF").unwrap_or_else(|| "main".into());

    if let Some(bin_path) = env_nonempty("LYRA_BIN_PATH") {
        *snapshot = snapshot_binary(&bin_path)?;
    }

    git_cmd(repo_dir, "git fetch", &["fetch", &remote]).await?;
    git_cmd(repo_dir, "git pull", &["pull", &remote, &git_ref]).await?;
    cargo_release_build(repo_dir).await?;
    crate::migrate::run_migrations(pool)
        .await
        .map_err(|e| format!("migrate failed: {e}"))?;

    if let Some(bin_path) = env_nonempty("LYRA_BIN_PATH") {
        install_release_binary(repo_dir, &bin_path)?;
    }

    Ok(format!("{remote}/{git_ref}"))
}

async fn maybe_restart() -> Result<(), String> {
    let Some(service) = env_nonempty("LYRA_SERVICE_NAME") else {
        return Ok(());
    };
    let mut cmd = Command::new("systemctl");
    cmd.arg("restart").arg(&service);
    run_cmd("systemctl restart", cmd).await
}

async fn revert_and_fail(
    pool: &SqlitePool,
    ticket_id: i64,
    snapshot: Option<&Snapshot>,
    err: &str,
) {
    let mut message = format!("Deploy failed: {err}");
    if let Some(snapshot) = snapshot
        && let Err(restore_err) = restore_binary(snapshot)
    {
        eprintln!("deploy ticket {ticket_id}: {restore_err}");
        message.push_str(&format!(" Binary restore failed: {restore_err}"));
    }
    eprintln!("deploy ticket {ticket_id}: {err}");
    let _ = insert_system_comment(pool, ticket_id, &message).await;
    let _ = set_status(pool, ticket_id, "failed").await;
}

async fn git_cmd(repo_dir: &str, step: &str, args: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo_dir).args(args);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    run_cmd(step, cmd).await
}

async fn cargo_release_build(repo_dir: &str) -> Result<(), String> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--release").current_dir(repo_dir);
    run_cmd("cargo build --release", cmd).await
}

async fn run_cmd(step: &str, mut cmd: Command) -> Result<(), String> {
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("{step} failed: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = stderr.trim();
    let detail = if detail.is_empty() {
        stdout.trim()
    } else {
        detail
    };
    match output.status.code() {
        Some(code) if detail.is_empty() => Err(format!("{step} failed (exit {code})")),
        Some(code) => Err(format!("{step} failed (exit {code}): {detail}")),
        None if detail.is_empty() => Err(format!("{step} failed")),
        None => Err(format!("{step} failed: {detail}")),
    }
}

async fn set_status(pool: &SqlitePool, ticket_id: i64, status: &str) -> Result<(), String> {
    sqlx::query(
        r#"UPDATE tickets SET "status" = $1, "updated_at" = datetime('now') WHERE "id" = $2"#,
    )
    .bind(status)
    .bind(ticket_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn insert_system_comment(
    pool: &SqlitePool,
    ticket_id: i64,
    text: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO comments ("ticket_id", "text", "author_name", "author_type", "format")
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(ticket_id)
    .bind(text)
    .bind("system")
    .bind("system")
    .bind("markdown")
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthError;
    use crate::jwt::encode_access_token;
    use crate::migrate::run_migrations;
    use crate::tickets::{self, Ticket};
    use axum::extract::{Path, State};
    use axum::http::{HeaderMap, HeaderValue, StatusCode};
    use axum::response::Json;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;
    use std::time::Duration;

    async fn setup_pool() -> sqlx::SqlitePool {
        clear_deploy_env();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
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
        sqlx::query(r#"INSERT INTO projects ("name", "description") VALUES ('p', 'd')"#)
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    fn clear_deploy_env() {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-jwt-secret");
            std::env::set_var("LYRA_REPO_DIR", "");
            std::env::set_var("LYRA_BIN_PATH", "");
            std::env::set_var("LYRA_SERVICE_NAME", "");
            std::env::set_var("LYRA_DEPLOY_GIT_REMOTE", "");
            std::env::set_var("LYRA_DEPLOY_GIT_REF", "");
        }
    }

    fn jwt_headers(user_type: &str, role: &str) -> HeaderMap {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-jwt-secret");
        }
        let token =
            encode_access_token("test-jwt-secret", "eyan", user_type, role, "Eyan").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        headers
    }

    fn human_headers() -> HeaderMap {
        jwt_headers("Human", "Owner")
    }

    fn agent_headers() -> HeaderMap {
        jwt_headers("Agent", "Engineer")
    }

    async fn insert_ticket(pool: &sqlx::SqlitePool, status: &str) -> i64 {
        sqlx::query_scalar(
            r#"INSERT INTO tickets ("name", "description", "project_id", "status")
               VALUES ('Task', 'Do it', 1, $1) RETURNING "id""#,
        )
        .bind(status)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn deploy(
        pool: &sqlx::SqlitePool,
        ticket_id: i64,
        headers: HeaderMap,
    ) -> Result<Json<Ticket>, (StatusCode, Json<AuthError>)> {
        tickets::deploy_ticket(State(Arc::new(pool.clone())), Path(ticket_id), headers).await
    }

    async fn wait_system_comment(pool: &sqlx::SqlitePool, ticket_id: i64) -> String {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            let text = sqlx::query_scalar::<_, String>(
                r#"SELECT "text" FROM comments
                   WHERE "ticket_id" = $1 AND "author_type" = 'system'
                   ORDER BY "id" DESC LIMIT 1"#,
            )
            .bind(ticket_id)
            .fetch_optional(pool)
            .await
            .unwrap();
            if let Some(text) = text {
                return text;
            }
            if tokio::time::Instant::now() > deadline {
                panic!("timed out waiting for deploy system comment");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn temp_name(label: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "lyra-step11-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        p
    }

    #[tokio::test]
    async fn deploy_without_jwt_is_unauthorized() {
        let pool = setup_pool().await;
        let id = insert_ticket(&pool, "pending_review").await;
        let err = deploy(&pool, id, HeaderMap::new()).await.err().unwrap();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert_eq!(err.1.0.error, "invalid_token");
    }

    #[tokio::test]
    async fn deploy_agent_jwt_is_forbidden() {
        let pool = setup_pool().await;
        let id = insert_ticket(&pool, "pending_review").await;
        let err = deploy(&pool, id, agent_headers()).await.err().unwrap();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        assert_eq!(err.1.0.error, "not_human");
    }

    #[tokio::test]
    async fn deploy_missing_ticket_is_not_found() {
        let pool = setup_pool().await;
        let err = deploy(&pool, 999, human_headers()).await.err().unwrap();
        assert_eq!(err.0, StatusCode::NOT_FOUND);
        assert_eq!(err.1.0.error, "not_found");
    }

    #[tokio::test]
    async fn deploy_queued_ticket_is_not_ready() {
        let pool = setup_pool().await;
        let id = insert_ticket(&pool, "queued").await;
        let err = deploy(&pool, id, human_headers()).await.err().unwrap();
        assert_eq!(err.0, StatusCode::CONFLICT);
        assert_eq!(err.1.0.error, "not_ready");
    }

    #[tokio::test]
    async fn deploy_skips_when_repo_dir_unset() {
        let pool = setup_pool().await;
        let id = insert_ticket(&pool, "pending_review").await;
        let ticket = deploy(&pool, id, human_headers()).await.unwrap();
        assert_eq!(ticket.status, "pending_review");

        let comment = wait_system_comment(&pool, id).await;
        assert!(
            comment.contains("LYRA_REPO_DIR"),
            "skip comment: {comment}"
        );

        let status: String =
            sqlx::query_scalar(r#"SELECT "status" FROM tickets WHERE "id" = $1"#)
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "pending_review");

        let format: String = sqlx::query_scalar(
            r#"SELECT "format" FROM comments WHERE "ticket_id" = $1 AND "author_type" = 'system'"#,
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(format, "markdown");
    }

    #[tokio::test]
    async fn deploy_human_with_empty_role_is_allowed() {
        let pool = setup_pool().await;
        let id = insert_ticket(&pool, "pending_review").await;
        let ticket = deploy(&pool, id, jwt_headers("Human", "")).await.unwrap();
        assert_eq!(ticket.status, "pending_review");
        let comment = wait_system_comment(&pool, id).await;
        assert!(comment.contains("LYRA_REPO_DIR"));
    }

    #[test]
    fn snapshot_skips_missing_binary() {
        let path = temp_name("missing-bin");
        let snap = snapshot_binary(path.to_str().unwrap()).unwrap();
        assert!(snap.is_none());
    }

    #[test]
    fn snapshot_and_restore_roundtrip() {
        let bin = temp_name("bin");
        std::fs::write(&bin, b"old-binary").unwrap();
        let snap = snapshot_binary(bin.to_str().unwrap()).unwrap().unwrap();
        std::fs::write(&bin, b"new-broken").unwrap();
        restore_binary(&snap).unwrap();
        assert_eq!(std::fs::read(&bin).unwrap(), b"old-binary");
        let _ = std::fs::remove_file(&bin);
        let _ = std::fs::remove_file(&snap.bak_path);
    }

    #[test]
    fn restore_fails_without_snapshot_file() {
        let bin = temp_name("restore-missing");
        let snap = Snapshot {
            bin_path: bin.to_string_lossy().into(),
            bak_path: temp_name("restore-missing.bak"),
        };
        let err = restore_binary(&snap).unwrap_err();
        assert!(err.contains("restore binary failed"), "{err}");
    }
}
