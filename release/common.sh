#!/usr/bin/env bash
set -euo pipefail

RELEASE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$RELEASE_DIR/.." && pwd)"

# shellcheck source=release.conf
source "$RELEASE_DIR/release.conf"

log() { printf '\n--- %s ---\n' "$1"; }
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

require_command() {
  local command_name
  for command_name in "$@"; do
    command -v "$command_name" >/dev/null 2>&1 || die "Required command is missing: $command_name"
  done
}

load_release_context() {
  TAG="${1:-}"
  [[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] || die "Usage: $0 vX.Y.Z"
  VERSION="${TAG#v}"

  RELEASE_WORK_BASE="${RELEASE_WORK_ROOT:-$(dirname "$ROOT")/release-work}"
  WORK_ROOT="$RELEASE_WORK_BASE/${PROJECT_SLUG}-${TAG}"
  ARTIFACTS_DIR="$WORK_ROOT/artifacts"
  SOURCE_DIR="$WORK_ROOT/source"
  SOURCE_ROOT="$SOURCE_DIR/${PROJECT_SLUG}-${TAG}"
  BUILD_DIR="$WORK_ROOT/build"
  STATE_DIR="$WORK_ROOT/state"
  FLATPAK_REPO="$WORK_ROOT/flatpak-repo"
  BUILD_LOG="$WORK_ROOT/flatpak-builder.log"
  BUILD_METADATA="$WORK_ROOT/build-metadata.env"
  QA_REPORT="$WORK_ROOT/qa-report.env"

  SOURCE_ZIP="${PROJECT_SLUG}-${TAG}.zip"
  SOURCE_CHECKSUM="${SOURCE_ZIP}.sha256"
  FLATPAK_BUNDLE="${PROJECT_SLUG}-${TAG}.flatpak"
  FLATPAK_CHECKSUM="${FLATPAK_BUNDLE}.sha256"

  SOURCE_ZIP_PATH="$ARTIFACTS_DIR/$SOURCE_ZIP"
  SOURCE_CHECKSUM_PATH="$ARTIFACTS_DIR/$SOURCE_CHECKSUM"
  FLATPAK_BUNDLE_PATH="$ARTIFACTS_DIR/$FLATPAK_BUNDLE"
  FLATPAK_CHECKSUM_PATH="$ARTIFACTS_DIR/$FLATPAK_CHECKSUM"
}

require_git_repository() {
  git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1 || die "Not inside a Git repository: $ROOT"
}

require_clean_worktree() {
  [[ -z "$(git -C "$ROOT" status --porcelain=v1)" ]] || die "Git worktree is not clean. Commit, stash, or remove local changes first."
}

require_local_tag() {
  git -C "$ROOT" rev-parse -q --verify "refs/tags/${TAG}^{commit}" >/dev/null || die "Local Git tag does not exist: $TAG"
}

tag_commit() { git -C "$ROOT" rev-parse "refs/tags/${TAG}^{commit}"; }

meson_project_version() {
  local version
  version="$(sed -nE "s/^[[:space:]]*version[[:space:]]*:[[:space:]]*['\"]([^'\"]+)['\"].*/\1/p" "$1" | head -n 1)"
  [[ -n "$version" ]] || die "No project version found in meson.build"
  printf '%s\n' "$version"
}

release_locales() { printf '%s\n' de es fr it nl pt; }

verify_locale_catalogs_in_root() {
  local files_root="$1" locale catalog
  while IFS= read -r locale; do
    catalog="$files_root/share/locale/$locale/LC_MESSAGES/git-desk.mo"
    [[ -s "$catalog" ]] || die "Installed locale catalog is missing or empty: $catalog"
  done < <(release_locales)
  printf 'Installed locale catalogs: PASS (de es fr it nl pt)\n'
}

require_artifacts() {
  local path
  for path in "$SOURCE_ZIP_PATH" "$SOURCE_CHECKSUM_PATH" "$FLATPAK_BUNDLE_PATH" "$FLATPAK_CHECKSUM_PATH"; do
    [[ -f "$path" ]] || die "Release artifact is missing: $path"
  done
}
