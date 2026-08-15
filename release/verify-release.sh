#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

load_release_context "${1:-}"
require_command git sha256sum unzip flatpak ostree mktemp grep
require_git_repository
require_local_tag
require_artifacts
TAG_COMMIT="$(tag_commit)"

log "CHECKSUM VERIFICATION"
(cd "$ARTIFACTS_DIR" && sha256sum --check "$SOURCE_CHECKSUM" && sha256sum --check "$FLATPAK_CHECKSUM")

log "SOURCE ARCHIVE COMMIT"
unzip -z "$SOURCE_ZIP_PATH" | grep -Fx "$TAG_COMMIT" >/dev/null || die "ZIP archive comment does not contain tag commit $TAG_COMMIT"
printf 'Archive commit: PASS (%s)\n' "$TAG_COMMIT"

log "SOURCE ARCHIVE ROOT"
ARCHIVE_PREFIX="${PROJECT_SLUG}-${TAG}/"
ARCHIVE_ENTRY_COUNT=0
while IFS= read -r entry; do
  [[ -n "$entry" ]] || continue
  ARCHIVE_ENTRY_COUNT=$((ARCHIVE_ENTRY_COUNT + 1))
  [[ "$entry" == "$ARCHIVE_PREFIX"* ]] || die "Archive contains path outside expected root: $entry"
done < <(unzip -Z1 "$SOURCE_ZIP_PATH")
(( ARCHIVE_ENTRY_COUNT > 0 )) || die "Archive is empty"
printf 'Archive root: PASS (%s)\n' "$ARCHIVE_PREFIX"

log "FLATPAK BUNDLE STRUCTURE"
AUDIT_ROOT="$(mktemp -d)"
trap 'rm -rf "$AUDIT_ROOT"' EXIT
ostree --repo="$AUDIT_ROOT/repo" init --mode=archive
IMPORT_LOG="$AUDIT_ROOT/build-import-bundle.log"
if ! flatpak build-import-bundle "$AUDIT_ROOT/repo" "$FLATPAK_BUNDLE_PATH" >"$IMPORT_LOG" 2>&1; then
  cat "$IMPORT_LOG"
  die "Flatpak bundle import failed"
fi
cat "$IMPORT_LOG"
[[ -f "$AUDIT_ROOT/repo/config" && -d "$AUDIT_ROOT/repo/objects" ]] || die "Bundle import did not create a valid OSTree repository"
[[ -f "$BUILD_METADATA" ]] || die "Build metadata is missing"

EXPECTED_REF="app/${APP_ID}/$(flatpak --default-arch)/${FLATPAK_BRANCH}"
ostree --repo="$AUDIT_ROOT/repo" refs | grep -Fx "$EXPECTED_REF" >/dev/null || die "Imported repository does not contain expected ref: $EXPECTED_REF"
EXPECTED_COMMIT="$(sed -n 's/^FLATPAK_COMMIT=//p' "$BUILD_METADATA")"
IMPORTED_COMMIT="$(ostree --repo="$AUDIT_ROOT/repo" rev-parse "$EXPECTED_REF")"
[[ -n "$EXPECTED_COMMIT" && "$IMPORTED_COMMIT" == "$EXPECTED_COMMIT" ]] || die "Imported bundle commit does not match build metadata"
printf 'Imported ref: PASS (%s)\nImported commit: PASS (%s)\nBundle import: PASS\n' "$EXPECTED_REF" "$IMPORTED_COMMIT"

printf '\nRelease verification: PASS\nNext: ./release/install-test.sh %s\n' "$TAG"
