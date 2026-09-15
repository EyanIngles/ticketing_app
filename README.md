# Building & Setup

This server provides secure access for the accompanying iOS application.

## Prerequisites

Before starting, ensure the following are installed:

* Tailscale
* Tailscale CLI
* A valid Tailscale account

## 1. Configure Environment Variables

An example environment file is provided. Copy it to create your own `.env` file:

```bash
cp exampledotenv .env
```

Open the newly created `.env` file and populate all required values.

> **Note:** Additional documentation for each environment variable will be provided in the future.

## 2. Configure HTTPS with Tailscale

To securely expose the server over HTTPS, Tailscale is used as the networking layer. This provides a simple, secure, and free solution for remote access.

Install Tailscale and authenticate your device.

Once connected, generate TLS certificates for your machine:

```bash
tailscale cert <tailscale-machine-name>
```

This command will generate the certificates required to serve HTTPS traffic.

## Next Steps

After generating the certificates:

1. Place the generated certificates in the appropriate location.
2. Update your `.env` file with the certificate paths if required.
3. Start the server.

## systemd (Pi)

Example units live in `systemd/`. Copy them to `/etc/systemd/system/`, edit `WorkingDirectory` / `ExecStart` / `EnvironmentFile` to the Pi paths, then:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now lyra opencode
```

`LYRA_SERVICE_NAME=lyra` in `.env` must match the Lyra unit name. Secrets stay in `.env`, not in the unit files. See `systemd/README.md`.

iOS API: `CONTRACT.md`. Pi tests: `scripts/pi-ci.sh`.

