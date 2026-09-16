#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="${LYRA_REPO_DIR:-$(cd "${SCRIPT_DIR}/.." && pwd)}"
cd "${REPO_DIR}"

cargo test --bin lyra-server
