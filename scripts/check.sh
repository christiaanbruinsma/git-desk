#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
pass() { printf 'PASS  %s\n' "$*"; }

for path in \
  Cargo.toml Cargo.lock meson.build meson_options.txt LICENSE README.md \
  io.github.christiaanbruinsma.GitDesk.yml \
  io.github.christiaanbruinsma.GitDesk.Devel.yml \
  data/io.github.christiaanbruinsma.GitDesk.desktop.in \
  data/io.github.christiaanbruinsma.GitDesk.metainfo.xml.in \
  data/icons/hicolor/scalable/apps/io.github.christiaanbruinsma.GitDesk.svg \
  po/LINGUAS po/de.po po/es.po po/fr.po po/it.po po/nl.po po/pt.po; do
  [[ -f "$path" ]] || fail "Missing required release file: $path"
done
pass "Required release files"

CARGO_VERSION="$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)"
MESON_VERSION="$(sed -nE "s/^[[:space:]]*version[[:space:]]*:[[:space:]]*'([^']+)'.*/\1/p" meson.build | head -1)"
META_VERSION="$(sed -nE 's/.*<release version="([^"]+)".*/\1/p' data/io.github.christiaanbruinsma.GitDesk.metainfo.xml.in | head -1)"
[[ -n "$CARGO_VERSION" && "$CARGO_VERSION" == "$MESON_VERSION" && "$CARGO_VERSION" == "$META_VERSION" ]] ||
  fail "Version mismatch: Cargo=$CARGO_VERSION Meson=$MESON_VERSION AppStream=$META_VERSION"
pass "Version sync ($CARGO_VERSION)"

grep -q '^app-id: io.github.christiaanbruinsma.GitDesk$' io.github.christiaanbruinsma.GitDesk.yml || fail "Production manifest app-id mismatch"
grep -q '^separate-locales: false$' io.github.christiaanbruinsma.GitDesk.yml || fail "Production standalone manifest must embed locales"
grep -q -- '- -Dprofile=production' io.github.christiaanbruinsma.GitDesk.yml || fail "Production manifest profile mismatch"

grep -q '^app-id: io.github.christiaanbruinsma.GitDesk.Devel$' io.github.christiaanbruinsma.GitDesk.Devel.yml || fail "Development manifest app-id mismatch"
grep -q '^separate-locales: true$' io.github.christiaanbruinsma.GitDesk.Devel.yml || fail "Development manifest must export .Locale"
grep -q -- '- -Dprofile=development' io.github.christiaanbruinsma.GitDesk.Devel.yml || fail "Development manifest profile mismatch"

[[ ! -e io.github.christiaanbruinsma.GitDesk.Release.yml ]] || fail "Legacy .Release manifest must not be present"
grep -q "choices: \['production', 'development'\]" meson_options.txt || fail "Meson profile choices mismatch"
grep -q "base_app_id = 'io.github.christiaanbruinsma.GitDesk'" meson.build || fail "Meson base app-id mismatch"
grep -q "base_app_id + '.Devel'" meson.build || fail "Meson development app-id wiring missing"
grep -q '@APP_ID@' data/io.github.christiaanbruinsma.GitDesk.desktop.in || fail "Desktop template app-id substitution missing"
grep -q '<id>@APP_ID@</id>' data/io.github.christiaanbruinsma.GitDesk.metainfo.xml.in || fail "AppStream template app-id substitution missing"
grep -q 'option_env!("GIT_DESK_APP_ID")' src/app.rs || fail "Rust profile app-id wiring missing"
pass "Production/development identity split"

grep -q 'gettext-rs' Cargo.toml || fail "gettext-rs dependency missing from Cargo.toml"
grep -q 'name = "gettext-rs"' Cargo.lock || fail "Cargo.lock is stale; build once in Builder and commit the updated lockfile"
pass "Committed gettext lock state"

EXPECTED_LINGUAS=$'de\nes\nfr\nit\nnl\npt'
ACTUAL_LINGUAS="$(grep -Ev '^[[:space:]]*(#|$)' po/LINGUAS)"
[[ "$ACTUAL_LINGUAS" == "$EXPECTED_LINGUAS" ]] || fail "po/LINGUAS does not match de/es/fr/it/nl/pt"
pass "Locale baseline (de es fr it nl pt)"

if command -v msgfmt >/dev/null 2>&1; then
  for locale in de es fr it nl pt; do
    msgfmt --check --check-format --output-file=/dev/null "po/$locale.po"
  done
  pass "gettext catalogs"
else
  printf 'INFO  msgfmt not installed on host; Flatpak build remains the authoritative catalog compile gate.\n'
fi

if [[ "${GIT_DESK_SKIP_CARGO:-0}" != "1" ]]; then
  command -v cargo >/dev/null 2>&1 || fail "cargo is required unless GIT_DESK_SKIP_CARGO=1"
  cargo fmt --check
  pass "cargo fmt"
fi

pass "Git Desk source release checks"
