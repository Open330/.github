#!/usr/bin/env bash

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BRANCH="${PROFILE_CRON_BRANCH:-main}"
DRY_RUN="${PROFILE_CRON_DRY_RUN:-0}"

if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=1
fi

log() {
  printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S %z')" "$*"
}

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

LOCK_DIR="/tmp/open330-profile-cron.lock"
if ! mkdir "$LOCK_DIR" 2>/dev/null; then
  log "[open330-cron] another run is active; exiting"
  exit 0
fi
trap 'rmdir "$LOCK_DIR"' EXIT

cd "$REPO_DIR"

for cmd in git gh cargo scc; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    log "[open330-cron] missing required command: $cmd"
    exit 1
  fi
done

if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  log "[open330-cron] tracked working tree is dirty; skipping run"
  exit 1
fi

current_branch="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$current_branch" != "$BRANCH" ]]; then
  log "[open330-cron] current branch '$current_branch' does not match '$BRANCH'"
  exit 1
fi

if ! token="$(gh auth token 2>/dev/null)"; then
  log "[open330-cron] failed to get GitHub token from gh auth"
  exit 1
fi
export GITHUB_TOKEN="$token"

log "[open330-cron] syncing '$BRANCH'"
git fetch origin "$BRANCH"
git rebase "origin/$BRANCH"

log "[open330-cron] generating profile README"
cargo run --release --manifest-path scripts/profile-gen/Cargo.toml

if git diff --quiet -- profile/README.md; then
  log "[open330-cron] no README changes"
  exit 0
fi

if [[ "$DRY_RUN" == "1" ]]; then
  log "[open330-cron] dry run complete; skipping commit/push"
  git restore -- profile/README.md
  exit 0
fi

git add profile/README.md
git commit -S -m "chore(profile): 🔄 update profile statistics"

log "[open330-cron] pushing updates"
git push origin "$BRANCH"
log "[open330-cron] completed"
