#!/usr/bin/env bash
set -euo pipefail
APP_ID="io.github.christiaanbruinsma.GitDesk"
LANGUAGE_CODE="${1:-}"

case "$LANGUAGE_CODE" in
  en) LOCALE="en_US.UTF-8" ;;
  nl) LOCALE="nl_NL.UTF-8" ;;
  de) LOCALE="de_DE.UTF-8" ;;
  fr) LOCALE="fr_FR.UTF-8" ;;
  es) LOCALE="es_ES.UTF-8" ;;
  it) LOCALE="it_IT.UTF-8" ;;
  pt) LOCALE="pt_PT.UTF-8" ;;
  *)
    printf 'Usage: %s {en|nl|de|fr|es|it|pt}\n' "$0" >&2
    exit 2
    ;;
esac

exec flatpak run --env="LANG=$LOCALE" --env="LANGUAGE=$LANGUAGE_CODE" "$APP_ID"
