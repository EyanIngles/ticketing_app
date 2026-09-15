# iOS ↔ Lyra API

Lyra on the Pi is the only API iOS talks to (Tailscale HTTPS). Lyra is the authorization server, not GitHub. JSON bodies. Errors: `{ "error": "..." }` (examples: `invalid_token`, `not_human`, `not_ready`, `already_decided`).

`POST /login` (302) is legacy web, not iOS.

## Auth

Humans sign in with PKCE S256, then use `Authorization: Bearer <access_token>`.

1. `POST /oauth/authorize`

```json
{ "username": "...", "password": "...", "client_id": "...", "code_challenge": "...", "code_challenge_method": "S256" }
```

→ `{ "code": "..." }` (single-use, ~10 min).

2. `POST /oauth/token` `grant_type=authorization_code`

```json
{ "grant_type": "authorization_code", "client_id": "...", "code": "...", "code_verifier": "..." }
```

→ `{ "access_token": "...", "refresh_token": "...", "token_type": "Bearer", "expires_in": 3600 }`.

3. Refresh: `{ "grant_type": "refresh_token", "refresh_token": "..." }` (same token response; old refresh is consumed).

`GET /current_user` — Bearer JWT. Body: `id`, `username`, `type`, `role`, `name`, `email`. JWT claims (conceptual): `username`, `type`, `role`, `name`.

## Tickets

| iOS | Endpoint |
| --- | --- |
| List / detail | `GET /tickets`, `GET /tickets/:id` |
| Submit task | `POST /tickets` `{ "name", "description", "project_id" }` |
| Markdown discuss | `POST /tickets/:id/comments` `{ "text" }` |
| Ask for PR | `POST /tickets/:id/actions/request_pr` |
| Close after merge | `POST /tickets/:id/actions/close` |
| Deploy | `POST /tickets/:id/actions/deploy` |

Ticket: `id`, `name`, `description`, `project_id`, `status`, `github_pr_url`, `last_model`, `comments[]`.

Comment: `id`, `text`, `author_name`, `author_type`, `author_role`, `model`, `format` (`markdown`), `display` (`Name:Model` for agents with a model, otherwise just name).

Statuses: `queued` → `running` → `awaiting_you` → `pr_opening` → `pending_review` → `closed` \| `failed`.

New tickets start `queued`. OpenCode dispatch may set `running` (skipped if `OPENCODE_BASE_URL` is unset). Dispatch failure → `failed` + system comment.

`request_pr` / `close` require JWT (any type) and return the ticket (`pr_opening` / `closed`). Missing ticket → 404 `{ "error": "not_found" }`.

Comments stamp JWT author when Bearer is present; `format` is `markdown`. Empty `text` → 400.

### Deploy

Human JWT only (`type=Human`; otherwise 403 `not_human`). Ticket must be `pending_review` else 409 `not_ready`. Returns the ticket immediately and runs deploy in the background.

Ping = poll ticket comments/status. No APNs. Success: system comment + `awaiting_you`. Failure: system comment + `failed`. If `LYRA_REPO_DIR` is unset, a system comment notes the skip and status stays `pending_review`.

## Permissions

Human JWT only.

| iOS | Endpoint |
| --- | --- |
| Pending | `GET /permissions?status=pending` (omit `status` → pending) |
| Allow / deny | `POST /permissions/:id/approve` or `/deny` |

Row: `id`, `ticket_id`, `author_*`, `model`, `display`, `tool`, `payload`, `status`, `opencode_session_id`, `permission_id`, `is_used`.

Approve/deny consume `is_used`. Second decision → 409 `already_decided`. Missing → 404 `not_found`. Lyra then tells OpenCode `once` (allow) or `reject` (deny), at most once per OpenCode `permission_id`.

## Agents (not iOS)

`POST /mcp` with `Authorization: Bearer <agent token>` and `X-Lyra-User: <username>`. JSON-RPC tools:

- Any agent: `lyra_get_ticket`, `lyra_add_comment`, `lyra_request_permission` (no human permission to call).
- Engineer only: `lyra_set_status`, `lyra_update_pr_url` (sets/replaces PR URL and status `pending_review`).

## Out of scope for iOS

GitHub, Dioxus UI, APNs, systemd, OpenCode bind (`127.0.0.1:4096`).
