#!/bin/sh
#
# Installeert kaartenjager. Geen root nodig, geen GitHub-account, geen compiler.
#
#   curl -fsSL https://raw.githubusercontent.com/yelsed/kaartenjager/main/install.sh | sh
#
set -eu

REPO="yelsed/kaartenjager"
BIN_DIR="${KAARTENJAGER_BIN_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${KAARTENJAGER_CONFIG_DIR:-$HOME/.config/kaartenjager}"
SKILL_DIR="${KAARTENJAGER_SKILL_DIR:-$HOME/.hermes/skills/kaartenjager}"
DATA_DIR="${KAARTENJAGER_DATA:-$HOME/.local/share/kaartenjager}"

say() { printf '%s\n' "$*"; }
die() { printf 'FOUT: %s\n' "$*" >&2; exit 1; }

need() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is nodig maar niet gevonden."
}

need curl
need tar

case "$(uname -s)" in
  Linux) ;;
  *) die "Alleen Linux wordt ondersteund; dit is $(uname -s)." ;;
esac

case "$(uname -m)" in
  x86_64|amd64)  TARGET=x86_64-unknown-linux-musl ;;
  aarch64|arm64) TARGET=aarch64-unknown-linux-musl ;;
  *) die "Onbekende architectuur $(uname -m). Bouw zelf met: cargo build --release" ;;
esac

say "==> Nieuwste versie opzoeken"
TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
      | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n1)
[ -n "$TAG" ] || die "Kon geen release vinden. Staat er al een gepubliceerd?"
say "    $TAG voor $TARGET"

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

ARCHIVE="kaartenjager-$TAG-$TARGET.tar.gz"
BASE="https://github.com/$REPO/releases/download/$TAG"

say "==> Downloaden"
curl -fsSL -o "$WORK/$ARCHIVE" "$BASE/$ARCHIVE" \
  || die "Downloaden mislukt: $BASE/$ARCHIVE"
curl -fsSL -o "$WORK/SHA256SUMS" "$BASE/SHA256SUMS" \
  || die "Kon SHA256SUMS niet ophalen."

say "==> Controlesom nakijken"
if command -v sha256sum >/dev/null 2>&1; then
  EXPECTED=$(grep " $ARCHIVE\$" "$WORK/SHA256SUMS" | cut -d' ' -f1)
  [ -n "$EXPECTED" ] || die "$ARCHIVE staat niet in SHA256SUMS."
  ACTUAL=$(sha256sum "$WORK/$ARCHIVE" | cut -d' ' -f1)
  [ "$EXPECTED" = "$ACTUAL" ] || die "Controlesom klopt niet. Download afgebroken."
  say "    in orde"
else
  say "    sha256sum ontbreekt — overgeslagen"
fi

say "==> Uitpakken naar $BIN_DIR"
tar xzf "$WORK/$ARCHIVE" -C "$WORK"
mkdir -p "$BIN_DIR"
install -m 0755 "$WORK/kaartenjager" "$BIN_DIR/kaartenjager"

say "==> Zelftest"
"$BIN_DIR/kaartenjager" selftest >/dev/null 2>&1 \
  || die "Zelftest gefaald. Draai '$BIN_DIR/kaartenjager selftest' voor de details."
say "    alle controles geslaagd"

say "==> Configuratie in $CONFIG_DIR"
mkdir -p "$CONFIG_DIR" "$DATA_DIR"
if [ -f "$CONFIG_DIR/kaartenjager.toml" ]; then
  say "    bestaande kaartenjager.toml blijft staan"
  curl -fsSL -o "$CONFIG_DIR/kaartenjager.toml.new" \
    "https://raw.githubusercontent.com/$REPO/$TAG/kaartenjager.example.toml" || true
  say "    nieuw voorbeeld ernaast: kaartenjager.toml.new"
else
  curl -fsSL -o "$CONFIG_DIR/kaartenjager.toml" \
    "https://raw.githubusercontent.com/$REPO/$TAG/kaartenjager.example.toml" \
    || die "Kon de voorbeeldconfiguratie niet ophalen."
  chmod 0600 "$CONFIG_DIR/kaartenjager.toml"
  say "    voorbeeldconfiguratie geplaatst"
fi

say "==> Hermes-skill in $SKILL_DIR"
mkdir -p "$SKILL_DIR/references"
for FILE in SKILL.md references/opstelling.md references/oordelen.md references/prijsherziening.md; do
  curl -fsSL -o "$SKILL_DIR/$FILE" \
    "https://raw.githubusercontent.com/$REPO/$TAG/hermes-skill/$FILE" \
    || say "    let op: $FILE kon niet opgehaald worden"
done

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) say ""; say "LET OP: $BIN_DIR staat niet in je PATH." ;;
esac

cat <<'DONE'

Klaar. Nog vier dingen:

  1. Vul je postcode in, anders werkt het ophaal-filter niet:
       nano ~/.config/kaartenjager/kaartenjager.toml

  2. Controleer en probeer:
       kaartenjager check
       kaartenjager run --dry-run

  3. Zet de twee cronjobs aan, met jouw kanaal erin. Vanuit Discord tegen Hermes:

       Maak twee cronjobs:
       - kaartenjager-scan, "*/5 8-22 * * *", no_agent, script
         ~/.local/bin/kaartenjager run, deliver naar dit kanaal
       - kaartenjager-prijzen, "0 9 * * 0", skill kaartenjager,
         prompt "Wekelijkse prijsherziening volgens de skill"

     Er is bewust geen cronjob voor het beoordelen: dat gebeurt op verzoek,
     vanuit de app.

  4. Kijken of ze draaien:
       hermes cron list

DONE
