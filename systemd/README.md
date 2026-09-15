# systemd units

Example units for the Pi. They contain **no secrets** and no real hostnames.

1. Copy `lyra.service` and `opencode.service` to `/etc/systemd/system/`.
2. Edit `WorkingDirectory`, `ExecStart`, and `EnvironmentFile` to the Pi paths (placeholders are `/opt/lyra/...`).
3. Put secrets only in the gitignored `.env` pointed at by `EnvironmentFile`.
4. Reload and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now lyra opencode
```

`LYRA_SERVICE_NAME=lyra` in `.env` must match the Lyra unit name so deploy can restart it.

OpenCode must stay on `127.0.0.1:4096`. iOS talks only to the Lyra API, never to OpenCode or GitHub.
