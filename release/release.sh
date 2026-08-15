#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'USAGE'
Usage:
  ./release/release.sh vX.Y.Z
  ./release/release.sh build vX.Y.Z
  ./release/release.sh verify vX.Y.Z
  ./release/release.sh test vX.Y.Z
  ./release/release.sh publish vX.Y.Z

Supplying only a tag runs build, verify and install-test. Publication still
requires typing PUBLISH after every local gate has passed.
USAGE
}

case "${1:-}" in
  v*)
    TAG="$1"
    "$SCRIPT_DIR/build-release.sh" "$TAG"
    "$SCRIPT_DIR/verify-release.sh" "$TAG"
    "$SCRIPT_DIR/install-test.sh" "$TAG"
    printf '\nAll local release gates passed for %s.\n' "$TAG"
    read -r -p "Type PUBLISH to create the GitHub Release: " confirmation
    [[ "$confirmation" == "PUBLISH" ]] || { printf 'Publication cancelled. Local artifacts remain available.\n'; exit 0; }
    exec "$SCRIPT_DIR/publish-release.sh" "$TAG"
    ;;
  build|verify|test|publish)
    MODE="$1"; TAG="${2:-}"
    case "$MODE" in
      build) exec "$SCRIPT_DIR/build-release.sh" "$TAG" ;;
      verify) exec "$SCRIPT_DIR/verify-release.sh" "$TAG" ;;
      test) exec "$SCRIPT_DIR/install-test.sh" "$TAG" ;;
      publish) exec "$SCRIPT_DIR/publish-release.sh" "$TAG" ;;
    esac
    ;;
  *) usage; exit 2 ;;
esac
