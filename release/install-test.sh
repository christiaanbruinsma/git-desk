#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

load_release_context "${1:-}"
require_command git flatpak sha256sum date
require_git_repository
require_local_tag
require_artifacts

ask_pass() {
  local prompt="$1" answer
  read -r -p "$prompt [y/N]: " answer
  [[ "$answer" =~ ^[Yy]$ ]]
}

write_qa_report() {
  local status="$1" installed_commit="${2:-}"
  cat > "$QA_REPORT" <<REPORT
STATUS=$status
PROJECT_SLUG=$PROJECT_SLUG
APP_ID=$APP_ID
TAG=$TAG
TAG_COMMIT=$(tag_commit)
FLATPAK_BUNDLE=$FLATPAK_BUNDLE
INSTALLED_COMMIT=$installed_commit
TESTED_AT_UTC=$(date -u +%Y-%m-%dT%H:%M:%SZ)
REPORT
}

flatpak info --user "$APP_ID" >/dev/null 2>&1 && die "$APP_ID is already installed in the user scope; uninstall the local test build first"
flatpak info --system "$APP_ID" >/dev/null 2>&1 && die "$APP_ID is already installed in the system scope"

log "VERIFY EXACT BUNDLE"
(cd "$ARTIFACTS_DIR" && sha256sum --check "$FLATPAK_CHECKSUM")

log "INSTALL EXACT RELEASE BUNDLE"
flatpak install --user --bundle "$FLATPAK_BUNDLE_PATH"
INSTALLED_COMMIT="$(flatpak info --user --show-commit "$APP_ID")"
INSTALL_LOCATION="$(flatpak info --user --show-location "$APP_ID")"
DEPLOYED_DESKTOP="$INSTALL_LOCATION/files/share/applications/${APP_ID}.desktop"
DEPLOYED_ICON="$INSTALL_LOCATION/files/share/icons/hicolor/scalable/apps/${APP_ID}.svg"
EXPORTED_DESKTOP="$HOME/.local/share/flatpak/exports/share/applications/${APP_ID}.desktop"
EXPORTED_ICON="$HOME/.local/share/flatpak/exports/share/icons/hicolor/scalable/apps/${APP_ID}.svg"
[[ -f "$DEPLOYED_DESKTOP" && -f "$DEPLOYED_ICON" ]] || die "Deployed desktop integration is incomplete"
[[ -e "$EXPORTED_DESKTOP" && -e "$EXPORTED_ICON" ]] || die "Host desktop exports are incomplete"

log "INSTALLED LOCALE CATALOGS"
verify_locale_catalogs_in_root "$INSTALL_LOCATION/files"

log "HOST GIT BRIDGE"
flatpak run --command=flatpak-spawn "$APP_ID" --host git --version
printf 'Host Git bridge: PASS\n'

if command -v desktop-file-validate >/dev/null 2>&1; then desktop-file-validate "$DEPLOYED_DESKTOP"; fi

log "RUNTIME SMOKE TEST"
printf 'The installed release bundle will start now. Complete the manual checks, then close Git Desk normally.\n\n'
flatpak run "$APP_ID"

MANUAL_PASS=true
ask_pass "Did Git Desk start and close without errors?" || MANUAL_PASS=false
ask_pass "Did the app menu, About dialog (v0.9.0), and Quit action work correctly?" || MANUAL_PASS=false
ask_pass "Could Git Desk open a local repository and show its Git status?" || MANUAL_PASS=false
ask_pass "Did Changes, History, Branches, Stashes, and Tags navigation work correctly?" || MANUAL_PASS=false
ask_pass "Were the desktop launcher and application icon correct?" || MANUAL_PASS=false
ask_pass "Did you verify NL/DE/FR/ES/IT/PT with scripts/run-locale.sh and see translated UI?" || MANUAL_PASS=false

log "UNINSTALL TEST INSTALLATION"
flatpak uninstall --user --noninteractive --delete-data "$APP_ID"
if flatpak info --user "$APP_ID" >/dev/null 2>&1; then write_qa_report "FAIL" "$INSTALLED_COMMIT"; die "Application is still installed after uninstall"; fi
[[ ! -e "$EXPORTED_DESKTOP" ]] || { write_qa_report "FAIL" "$INSTALLED_COMMIT"; die "Desktop export still exists after uninstall"; }
[[ ! -e "$EXPORTED_ICON" ]] || { write_qa_report "FAIL" "$INSTALLED_COMMIT"; die "Icon export still exists after uninstall"; }
(cd "$ARTIFACTS_DIR" && sha256sum --check "$SOURCE_CHECKSUM" && sha256sum --check "$FLATPAK_CHECKSUM")

if [[ "$MANUAL_PASS" != true ]]; then write_qa_report "FAIL" "$INSTALLED_COMMIT"; die "One or more manual acceptance checks failed"; fi
write_qa_report "PASS" "$INSTALLED_COMMIT"
log "QA REPORT"
cat "$QA_REPORT"
printf '\nInstall test: PASS\nNext: ./release/publish-release.sh %s\n' "$TAG"
