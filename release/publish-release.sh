#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

TAG_ARGUMENT="${1:-}"
load_release_context "$TAG_ARGUMENT"
require_command git gh sha256sum cmp mktemp awk grep
require_git_repository
require_clean_worktree
require_local_tag
require_artifacts

"$SCRIPT_DIR/verify-release.sh" "$TAG"
[[ -f "$QA_REPORT" ]] || die "QA report is missing. Run ./release/install-test.sh $TAG first."
grep -Fx "STATUS=PASS" "$QA_REPORT" >/dev/null || die "QA report is not PASS"
TAG_COMMIT="$(tag_commit)"
grep -Fx "TAG_COMMIT=$TAG_COMMIT" "$QA_REPORT" >/dev/null || die "QA report belongs to a different tag commit"

log "REMOTE TAG VERIFICATION"
REMOTE_TAG_COMMIT="$(git -C "$ROOT" ls-remote "$RELEASE_REMOTE" "refs/tags/${TAG}^{}" | awk 'NR == 1 {print $1}')"
if [[ -z "$REMOTE_TAG_COMMIT" ]]; then
  REMOTE_TAG_COMMIT="$(git -C "$ROOT" ls-remote "$RELEASE_REMOTE" "refs/tags/${TAG}" | awk 'NR == 1 {print $1}')"
fi
[[ -n "$REMOTE_TAG_COMMIT" ]] || die "Remote tag does not exist: $TAG"
[[ "$REMOTE_TAG_COMMIT" == "$TAG_COMMIT" ]] || die "Local and remote tag commits differ"
printf 'Remote tag: PASS (%s)\n' "$REMOTE_TAG_COMMIT"

log "GITHUB AUTHENTICATION"
gh auth status

ASSET_PATHS=("$SOURCE_ZIP_PATH" "$SOURCE_CHECKSUM_PATH" "$FLATPAK_BUNDLE_PATH" "$FLATPAK_CHECKSUM_PATH")
ASSET_NAMES=("$SOURCE_ZIP" "$SOURCE_CHECKSUM" "$FLATPAK_BUNDLE" "$FLATPAK_CHECKSUM")
TEMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TEMP_ROOT"' EXIT

NOTES_FILE="$TEMP_ROOT/release-notes.md"
SOURCE_SHA256="$(awk '{print $1}' "$SOURCE_CHECKSUM_PATH")"
FLATPAK_SHA256="$(awk '{print $1}' "$FLATPAK_CHECKSUM_PATH")"
cat > "$NOTES_FILE" <<NOTES
# $PROJECT_NAME $TAG

**Easy to start. Powerful enough to stay.**

First public preview of Git Desk, a native GNOME Git client built with Rust, GTK4 and libadwaita.

## Highlights

- Repository opening, initialization and cloning
- Changes, staging, commits, amend and undo workflows
- History, branches, merge recovery, remotes, fetch/pull/push
- Stashes and tags
- Revert and cherry-pick recovery flows
- EN, NL, DE, FR, ES, IT and PT UI baseline

## Install

Verify the Flatpak:

\`\`\`bash
sha256sum --check $FLATPAK_CHECKSUM
\`\`\`

Install for the current user:

\`\`\`bash
flatpak install --user ./$FLATPAK_BUNDLE
\`\`\`

Start:

\`\`\`bash
flatpak run $APP_ID
\`\`\`

## SHA-256

\`\`\`text
$FLATPAK_SHA256  $FLATPAK_BUNDLE
$SOURCE_SHA256  $SOURCE_ZIP
\`\`\`

Git Desk uses the host Git installation so it can work with the user's existing Git configuration and credentials.
NOTES

if gh release view "$TAG" --repo "$GITHUB_REPOSITORY" >/dev/null 2>&1; then
  die "GitHub release already exists for $TAG. Preserve published release immutability and use a new version/tag for changes."
fi

log "CREATE GITHUB RELEASE"
gh release create "$TAG" --repo "$GITHUB_REPOSITORY" --verify-tag --title "$PROJECT_NAME $TAG" --notes-file "$NOTES_FILE" "${ASSET_PATHS[@]}"

log "FINAL PUBLISHED-ASSET AUDIT"
for index in "${!ASSET_NAMES[@]}"; do
  asset_name="${ASSET_NAMES[$index]}"
  asset_path="${ASSET_PATHS[$index]}"
  asset_dir="$TEMP_ROOT/final-$index"
  mkdir -p "$asset_dir"
  gh release download "$TAG" --repo "$GITHUB_REPOSITORY" --pattern "$asset_name" --dir "$asset_dir"
  cmp --silent "$asset_path" "$asset_dir/$asset_name" || die "Published asset does not match local artifact: $asset_name"
  printf 'PASS  Published asset matches: %s\n' "$asset_name"
done

gh release view "$TAG" --repo "$GITHUB_REPOSITORY" --json tagName,name,isDraft,isPrerelease,url,assets --jq '{tag:.tagName,name:.name,draft:.isDraft,prerelease:.isPrerelease,url:.url,assets:[.assets[]|{name:.name,size:.size,state:.state}]}'
printf '\nGitHub publication: PASS\n'
