#!/usr/bin/env bash
# audit.sh — BAD_RUST scan.
# READ-ONLY: the edit-agent must not modify this file.

set -euo pipefail
cd "$(dirname "$0")/.."

if [ -x "$HOME/.claude/skills/autobuilder/rules/audit-checks.sh" ]; then
  exec "$HOME/.claude/skills/autobuilder/rules/audit-checks.sh" .
else
  echo '{"blocking_count":0,"advisory_count":0,"notes":"audit-checks.sh not found"}' > target/autobuilder/audit.json
fi
