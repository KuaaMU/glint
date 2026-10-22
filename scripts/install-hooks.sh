#!/usr/bin/env bash
# Install the local pre-push hook. Run once per clone.
#
# A pre-commit hook would run the full suite on every commit, which is too slow
# while iterating. Pre-push is the last gate before CI sees the branch, which is
# where a cheap local check pays off.
set -euo pipefail

cd "$(dirname "$0")/.."

hook=".git/hooks/pre-push"
cat > "$hook" <<'HOOK'
#!/usr/bin/env bash
# Installed by scripts/install-hooks.sh
set -euo pipefail
echo "pre-push: running scripts/check.sh (skip with --no-verify)"
exec ./scripts/check.sh
HOOK
chmod +x "$hook"

echo "Installed $hook"
echo "Bypass once with: git push --no-verify"
