#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$ROOT_DIR"

if [[ -x "$PROJECT_ROOT/venv/bin/python" ]]; then
  PYTHON_BIN="$PROJECT_ROOT/venv/bin/python"
else
  if command -v python3 >/dev/null 2>&1; then
    PYTHON_BIN="$(command -v python3)"
  else
    PYTHON_BIN="$(command -v python)"
  fi
fi

if [[ -z "${PYTHON_BIN:-}" ]]; then
  echo "Python interpreter not found" >&2
  exit 1
fi

HASH="$($PYTHON_BIN - <<'PY'
from werkzeug.security import generate_password_hash
print(generate_password_hash('owner'), end='')
PY
)"

cat > "$PROJECT_ROOT/users.json" <<EOF
{
  "owner": {
    "password": "$HASH",
    "role": "owner",
    "assigned_instances": []
  }
}
EOF

echo "Created users.json with default owner credentials (owner / owner)."
