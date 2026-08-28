#!/bin/sh
# Elke knop uitproberen tegen de PRODUCTIEBOUW, met een Origin-kop.
#
# Dat allebei is het punt. De CSRF-controle van SvelteKit staat uit in ontwikkelmodus
# (`if (!__SVELTEKIT_DEV__)` in runtime/server/respond.js), en zonder Origin-kop lijkt een
# POST van curl nergens op wat een browser doet. Een test die dat allebei negeert gaf hier
# vier groene vinkjes voor knoppen die in productie allemaal 403 kregen.
#
#   npm run smoke
#
# Draait op een eigen wegwerpdatabase; de echte gegevens blijven onaangeraakt.

set -eu

PORT="${SMOKE_PORT:-5199}"
ORIGIN="http://localhost:$PORT"
WORK="$(mktemp -d)"
DB="$WORK/kaartenjager.db"
SERVER_PID=""
FAILURES=0

cleanup() {
  [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

say() { printf '%s\n' "$*"; }
fout() { printf 'FOUT  %s\n' "$*"; FAILURES=$((FAILURES + 1)); }

# ------------------------------------------------------------------ database

# Het schema hoort bij het programma, dus dat maakt hem aan — precies zoals in productie.
BIN="${KAARTENJAGER_BIN:-$HOME/.local/bin/kaartenjager}"
[ -x "$BIN" ] || BIN="../target/release/kaartenjager"
[ -x "$BIN" ] || { say "Geen kaartenjager-programma gevonden; bouw het eerst met cargo build --release."; exit 1; }

cat > "$WORK/kaartenjager.toml" <<'TOML'
card_search_terms = ["rtx 3090"]
part_search_terms = []

[[card]]
name = "RTX 3090"
patterns = ["3090"]
vram_gb = 24
used_price_low = 750
used_price_high = 925
alert_below = 700
suspicious_below = 450
TOML

# `check` maakt de database aan en zaait de zoektermen, net als bij de eerste start.
KAARTENJAGER_DB="$DB" KAARTENJAGER_CONFIG="$WORK/kaartenjager.toml" "$BIN" check > /dev/null

# Eén advertentie met een vondst erbij, zodat de knoppen iets hebben om op te werken.
sqlite3 "$DB" <<'SQL'
INSERT INTO listing (key, source, listing_id, title, url, first_seen, last_seen)
  VALUES ('vinted:1', 'vinted', '1', 'RTX 3090 test', 'https://example.invalid/1', 1000, 1000);
INSERT INTO sighting (key, seen_at, price_cents, asking_cents, favourite_count)
  VALUES ('vinted:1', 1000, 60000, 60000, 3);
INSERT INTO finding (key, matched_as, kind, confidence, reasons, warnings,
                     became_a_find_at, judged_at)
  VALUES ('vinted:1', 'RTX 3090', 'card', 'clear', '[]', '[]', 1000, 1000);
INSERT INTO decision (key, state, changed_at) VALUES ('vinted:1', 'inbox', 1000);
SQL

# -------------------------------------------------------------------- server

# De knop "nu zoeken" start een los proces. Alleen `run` wordt hier nagebootst, want deze
# proef gaat over de app en niet over de zoekmachine — het echte programma heeft zijn eigen
# zelftest. De stub doet er even over, zodat ook te zien is dat er niet twee rondes tegelijk
# starten.
#
# Al het andere gaat door naar het echte programma. Het instellingenscherm laat `check` en
# `config path` erdoor draaien, en juist dát is wat het scherm veilig maakt: een nagebootste
# controle die alles goedkeurt zou de proef groen maken voor een scherm dat in productie
# kapotte configuratie wegschrijft.
cat > "$WORK/nep-kaartenjager" <<STUB
#!/bin/sh
case "\$1" in
  run) sleep 2 ;;
  *) exec "$BIN" "\$@" ;;
esac
STUB
chmod +x "$WORK/nep-kaartenjager"

say "==> Productiebouw starten op poort $PORT"
KAARTENJAGER_DB="$DB" PORT="$PORT" KAARTENJAGER_BIN="$WORK/nep-kaartenjager" \
  KAARTENJAGER_CONFIG="$WORK/kaartenjager.toml" \
  node build > "$WORK/server.log" 2>&1 &
SERVER_PID=$!

WACHT=0
until curl -sf "http://localhost:$PORT/" -o /dev/null 2>/dev/null; do
  WACHT=$((WACHT + 1))
  [ "$WACHT" -gt 100 ] && { say "De server kwam niet op:"; cat "$WORK/server.log"; exit 1; }
  sleep 0.1
done

# ------------------------------------------------------------------- acties

# $1 pad, $2 actie, $3 verwachte status, rest: formuliervelden
probeer() {
  pad="$1"; actie="$2"; verwacht="$3"; shift 3
  status=$(curl -sS -o "$WORK/antwoord.html" -w '%{http_code}' -X POST \
    -H "Origin: $ORIGIN" "$@" "http://localhost:$PORT$pad?/$actie")
  if [ "$status" = "$verwacht" ]; then
    say "ok    $actie ($status)"
  else
    fout "$actie gaf $status, verwacht $verwacht"
  fi
}

# Een geweigerde actie geeft bij een gewone formulierpost gewoon 200 met de melding in de
# pagina; de status uit fail() geldt alleen voor de fetch die use:enhance doet. Dus toetsen
# we op wat de gebruiker te zien krijgt.
zegt() {
  wat="$1"
  if grep -qF "$wat" "$WORK/antwoord.html"; then
    say "ok    de app zegt \"$wat\""
  else
    fout "de app zei niet \"$wat\""
  fi
}

say "==> De knoppen"
probeer "/" volgen 200 --data-urlencode 'key=vinted:1'
[ "$(sqlite3 "$DB" "SELECT state FROM decision WHERE key='vinted:1';")" = "watching" ] \
  || fout "volgen zette de staat niet op watching"

probeer "/" archiveren 200 --data-urlencode 'key=vinted:1'
[ "$(sqlite3 "$DB" "SELECT state FROM decision WHERE key='vinted:1';")" = "archived" ] \
  || fout "archiveren zette de staat niet op archived"
[ -n "$(sqlite3 "$DB" "SELECT price_when_archived FROM decision WHERE key='vinted:1';")" ] \
  || fout "archiveren onthield de prijs niet"

probeer "/archief" terug 200 --data-urlencode 'key=vinted:1'
[ "$(sqlite3 "$DB" "SELECT state FROM decision WHERE key='vinted:1';")" = "inbox" ] \
  || fout "terug zette de staat niet op inbox"

probeer "/" hermes 200 --data-urlencode 'key=vinted:1'
[ "$(sqlite3 "$DB" "SELECT COUNT(*) FROM review_request;")" = "1" ] \
  || fout "de Hermes-knop zette geen verzoek in de wachtrij"
probeer "/" hermes 200 --data-urlencode 'key=vinted:1'
[ "$(sqlite3 "$DB" "SELECT COUNT(*) FROM review_request;")" = "1" ] \
  || fout "tweemaal drukken leverde meer dan één verzoek op"

probeer "/" allesGezien 200 --data 'x=1'
[ -n "$(sqlite3 "$DB" "SELECT value FROM app_state WHERE name='last_visit';")" ] \
  || fout "alles gezien legde het bezoek niet vast"

say "==> De zoektermen"
probeer "/zoektermen" toevoegen 200 --data-urlencode 'term=rtx 4090 test' --data-urlencode 'kind=card'
[ "$(sqlite3 "$DB" "SELECT COUNT(*) FROM search_term WHERE term='rtx 4090 test';")" = "1" ] \
  || fout "de zoekterm werd niet toegevoegd"
probeer "/zoektermen" toevoegen 200 --data-urlencode 'term=rtx 4090 test' --data-urlencode 'kind=card'
zegt "staat er al"
[ "$(sqlite3 "$DB" "SELECT COUNT(*) FROM search_term WHERE term='rtx 4090 test';")" = "1" ] \
  || fout "een dubbele zoekterm kwam er toch bij"
probeer "/zoektermen" verwijderen 200 --data-urlencode 'term=rtx 4090 test'

say "==> Zelf een ronde starten"
probeer "/" nuZoeken 200 --data 'x=1'
zegt "Ronde gestart"
# Twee rondes naast elkaar leveren niets extra's op en verdubbelen wel het aantal verzoeken
# aan Vinted en Marktplaats.
probeer "/" nuZoeken 200 --data 'x=1'
zegt "Er loopt al een ronde"

say "==> De instellingen"
curl -sS "http://localhost:$PORT/instellingen" -o "$WORK/instellingen.html"
grep -qF "$WORK/kaartenjager.toml" "$WORK/instellingen.html" \
  || fout "het instellingenscherm toont niet het bestand dat de wachter leest"
grep -qF "alert_below" "$WORK/instellingen.html" \
  || fout "het instellingenscherm toont de inhoud niet"

# Kapotte TOML mag nooit weggeschreven worden: dat is de enige reden dat dit scherm mag
# bestaan. Zonder deze controle is het een knop die de wachter vannacht stilzet.
VOOR="$(cat "$WORK/kaartenjager.toml")"
probeer "/instellingen" bewaren 200 --data-urlencode 'inhoud=dit is [geen geldige toml'
zegt "keurde dit af"
[ "$(cat "$WORK/kaartenjager.toml")" = "$VOOR" ] \
  || fout "kapotte TOML werd tóch weggeschreven"
[ ! -f "$WORK/kaartenjager.toml.nieuw" ] \
  || fout "de afgekeurde versie bleef als .nieuw achter"

# Ook een bestand dat wél geldige TOML is maar door `check` afgekeurd wordt — hier een
# kaartregel met een drempel boven het marktbereik — hoort te blijven liggen.
probeer "/instellingen" bewaren 200 --data-urlencode 'inhoud=card_search_terms = []
part_search_terms = []'
[ "$(cat "$WORK/kaartenjager.toml")" = "$VOOR" ] \
  || fout "een configuratie zonder zoektermen werd weggeschreven"

probeer "/instellingen" bewaren 200 --data-urlencode "inhoud=$VOOR
# door de proef toegevoegd"
grep -qF "door de proef toegevoegd" "$WORK/kaartenjager.toml" \
  || fout "een goedgekeurde wijziging werd niet weggeschreven"
[ -f "$WORK/kaartenjager.toml.vorige" ] \
  || fout "er werd geen vorige versie bewaard"
[ "$(cat "$WORK/kaartenjager.toml.vorige")" = "$VOOR" ] \
  || fout "de bewaarde vorige versie klopt niet"

say "==> De bescherming"
probeer_vreemd() {
  status=$(curl -sS -o /dev/null -w '%{http_code}' -X POST \
    -H 'Origin: https://evil.example' --data-urlencode 'key=vinted:1' \
    "http://localhost:$PORT/?/volgen")
  if [ "$status" = "403" ]; then
    say "ok    vreemde herkomst geweigerd (403)"
  else
    fout "een vreemde herkomst gaf $status, verwacht 403"
  fi
}
probeer_vreemd

status=$(curl -sS -o /dev/null -w '%{http_code}' -X POST \
  --data-urlencode 'key=vinted:1' "http://localhost:$PORT/?/volgen")
if [ "$status" = "403" ]; then
  say "ok    zonder Origin-kop geweigerd (403)"
else
  fout "zonder Origin-kop gaf $status, verwacht 403"
fi

# --------------------------------------------------------------------- slot

if [ "$FAILURES" -eq 0 ]; then
  say ""
  say "Alles goed."
else
  say ""
  say "$FAILURES controle(s) mislukt. Serverlog:"
  tail -20 "$WORK/server.log"
  exit 1
fi
