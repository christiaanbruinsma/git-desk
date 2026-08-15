#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

load_release_context "${1:-}"
require_command git sha256sum unzip flatpak flatpak-builder tee ostree
require_git_repository
require_clean_worktree
require_local_tag

TAG_COMMIT="$(tag_commit)"

log "RELEASE IDENTITY"
printf 'Project:       %s\n' "$PROJECT_NAME"
printf 'Tag:           %s\n' "$TAG"
printf 'Version:       %s\n' "$VERSION"
printf 'Tag commit:    %s\n' "$TAG_COMMIT"
printf 'Workspace:     %s\n' "$WORK_ROOT"

log "CLEAN RELEASE WORKSPACE"
mkdir -p "$ARTIFACTS_DIR"
rm -rf -- "$SOURCE_DIR" "$BUILD_DIR" "$STATE_DIR" "$FLATPAK_REPO"
rm -f -- "$SOURCE_ZIP_PATH" "$SOURCE_CHECKSUM_PATH" "$FLATPAK_BUNDLE_PATH" "$FLATPAK_CHECKSUM_PATH" "$BUILD_LOG" "$BUILD_METADATA" "$QA_REPORT"
mkdir -p "$SOURCE_DIR"

log "SOURCE ARCHIVE FROM GIT TAG"
git -C "$ROOT" archive --format=zip --prefix="${PROJECT_SLUG}-${TAG}/" --output="$SOURCE_ZIP_PATH" "$TAG"
(
  cd "$ARTIFACTS_DIR"
  sha256sum "$SOURCE_ZIP" > "$SOURCE_CHECKSUM"
  sha256sum --check "$SOURCE_CHECKSUM"
)

log "EXTRACT TAGGED SOURCE"
unzip -q "$SOURCE_ZIP_PATH" -d "$SOURCE_DIR"
[[ -f "$SOURCE_ROOT/$MANIFEST" ]] || die "Manifest is missing from tagged source: $MANIFEST"
[[ -f "$SOURCE_ROOT/meson.build" ]] || die "meson.build is missing from tagged source"
[[ -f "$SOURCE_ROOT/Cargo.lock" ]] || die "Cargo.lock is missing from tagged source"

grep -q 'name = "gettext-rs"' "$SOURCE_ROOT/Cargo.lock" || die "Tagged Cargo.lock does not include gettext-rs"

ARCHIVE_VERSION="$(meson_project_version "$SOURCE_ROOT/meson.build")"
[[ "$ARCHIVE_VERSION" == "$VERSION" ]] || die "Tag version $VERSION does not match Meson version $ARCHIVE_VERSION"
printf 'Version contract: PASS (%s)\n' "$ARCHIVE_VERSION"

if [[ -n "${QUALITY_COMMAND:-}" ]]; then
  log "TAGGED SOURCE QUALITY CHECKS"
  (cd "$SOURCE_ROOT" && bash -lc "$QUALITY_COMMAND")
fi

log "RESTORE PRISTINE TAGGED SOURCE"
rm -rf -- "$SOURCE_DIR"
mkdir -p "$SOURCE_DIR"
unzip -q "$SOURCE_ZIP_PATH" -d "$SOURCE_DIR"
[[ -f "$SOURCE_ROOT/$MANIFEST" ]] || die "Manifest is missing after restoring tagged source"
[[ -f "$SOURCE_ROOT/Cargo.lock" ]] || die "Cargo.lock is missing after restoring tagged source"
printf 'Pristine tagged build source: PASS\n'

log "FLATPAK BUILD"
flatpak-builder --force-clean --state-dir="$STATE_DIR" --repo="$FLATPAK_REPO" "$BUILD_DIR" "$SOURCE_ROOT/$MANIFEST" 2>&1 | tee "$BUILD_LOG"
[[ -d "$BUILD_DIR/files" ]] || die "Flatpak build directory was not created"
[[ -d "$FLATPAK_REPO/objects" ]] || die "Flatpak repository was not created"

log "LOCALE ARTIFACTS"
verify_locale_catalogs_in_root "$BUILD_DIR/files"

log "DESKTOP ASSETS"
[[ -f "$BUILD_DIR/files/share/applications/${APP_ID}.desktop" ]] || die "Desktop file missing from build"
[[ -f "$BUILD_DIR/files/share/icons/hicolor/scalable/apps/${APP_ID}.svg" ]] || die "Application icon missing from build"
[[ -f "$BUILD_DIR/files/share/metainfo/${APP_ID}.metainfo.xml" ]] || die "AppStream metadata missing from build"
printf 'Desktop assets: PASS\n'

flatpak build-update-repo "$FLATPAK_REPO"

log "SINGLE-FILE FLATPAK BUNDLE"
flatpak build-bundle --runtime-repo="$RUNTIME_REPO_URL" "$FLATPAK_REPO" "$FLATPAK_BUNDLE_PATH" "$APP_ID" "$FLATPAK_BRANCH"
(
  cd "$ARTIFACTS_DIR"
  sha256sum "$FLATPAK_BUNDLE" > "$FLATPAK_CHECKSUM"
  sha256sum --check "$FLATPAK_CHECKSUM"
)

FLATPAK_COMMIT="$(ostree --repo="$FLATPAK_REPO" rev-parse "app/$APP_ID/$(flatpak --default-arch)/$FLATPAK_BRANCH")"
cat > "$BUILD_METADATA" <<META
STATUS=PASS
PROJECT_SLUG=$PROJECT_SLUG
APP_ID=$APP_ID
TAG=$TAG
VERSION=$VERSION
TAG_COMMIT=$TAG_COMMIT
FLATPAK_BRANCH=$FLATPAK_BRANCH
FLATPAK_COMMIT=$FLATPAK_COMMIT
SOURCE_ZIP=$SOURCE_ZIP
FLATPAK_BUNDLE=$FLATPAK_BUNDLE
META

log "RELEASE ARTIFACTS"
ls -lh "$SOURCE_ZIP_PATH" "$SOURCE_CHECKSUM_PATH" "$FLATPAK_BUNDLE_PATH" "$FLATPAK_CHECKSUM_PATH"

printf '\nRelease build: PASS\nNext: ./release/verify-release.sh %s\n' "$TAG"
