#!/bin/sh
# Kaartenjager uitrollen of bijwerken: het programma, de configuratie, de database en de app.
#
# Bedoeld om ongewijzigd te draaien, ook als er al iets staat. Elke stap kijkt eerst of hij
# nodig is, en niets wat van jou is wordt overschreven — niet je kaartenjager.toml en niet een
# service-bestand dat je zelf hebt aangepast.
#
#   sh deploy.sh
#
# Vooraf in te vullen als je wilt dat de Hermes-knop meteen werkt:
#   KAARTENJAGER_DISCORD_WEBHOOK=https://discord.com/api/webhooks/... sh deploy.sh
#
# Zonder die webhook werkt alles, alleen wordt Hermes niet gewekt als je op de knop drukt.
# Het verzoek komt dan wel in de wachtrij en de scan meldt het na een kwartier alsnog.

set -eu

REPO_URL="https://github.com/yelsed/kaartenjager.git"
CHECKOUT="${KAARTENJAGER_CHECKOUT:-$HOME/kaartenjager}"
CONFIG_DIR="${KAARTENJAGER_CONFIG_DIR:-$HOME/.config/kaartenjager}"
CONFIG="$CONFIG_DIR/kaartenjager.toml"
BIN="${KAARTENJAGER_BIN_DIR:-$HOME/.local/bin}/kaartenjager"
UNIT_DIR="$HOME/.config/systemd/user"
UNIT="$UNIT_DIR/kaartenjager-app.service"
APP_PORT="${KAARTENJAGER_APP_PORT:-5173}"

say() { printf '%s\n' "$*"; }
step() { printf '\n==> %s\n' "$*"; }
die() { printf 'GESTOPT: %s\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------- 1. gereedschap

step "Gereedschap controleren"
for tool in curl git; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool ontbreekt."
done

command -v node >/dev/null 2>&1 || die "node ontbreekt. De app heeft Node 22 of nieuwer nodig."
NODE_MAJOR=$(node --version | sed 's/^v\([0-9]*\).*/\1/')
if [ "$NODE_MAJOR" -lt 22 ]; then
  die "Node $NODE_MAJOR is te oud. De app leest de database met node:sqlite, en dat zit pas
     vanaf Node 22 in Node zelf."
fi
say "    node $(node --version), npm $(npm --version)"

# ------------------------------------------------------------------- 2. programma

step "Het programma installeren of bijwerken"
curl -fsSL "https://raw.githubusercontent.com/yelsed/kaartenjager/main/install.sh" | sh
say "    versie nu: $("$BIN" --version)"

# ---------------------------------------------------------------- 3. configuratie

step "Configuratie nalopen"
[ -f "$CONFIG" ] || die "$CONFIG bestaat niet. Draai install.sh eerst met de hand."

if grep -q '^\[notify\]' "$CONFIG"; then
  say "    [notify] staat er al"
else
  # Onderaan toevoegen, en met opzet niet ergens middenin: een tabelkop slikt alle losse
  # sleutels die erna komen op. Zet je [notify] boven card_search_terms, dan verdwijnen je
  # zoektermen erin en start het programma niet meer.
  cat >> "$CONFIG" <<'TOML'

# Wanneer een vondst het waard is om je voor te storen. Alles wat het programma
# vindt komt in de app; alleen echte uitschieters gaan ook naar Discord.
[notify]
push_below_market_percent = 35
TOML
  say "    [notify] onderaan toegevoegd, drempel op 35%"
fi

if grep -q '^\[scan\]' "$CONFIG"; then
  say "    [scan] staat er al"
else
  cat >> "$CONFIG" <<'TOML'

# Hoe de ronde zich verdeelt over zoeken en volgen. Zoeken hoort vaak te
# gebeuren, prijzen volgen van advertenties die je al kent juist niet.
[scan]
recheck_every_minutes = 30
close_watch_hours = 6
TOML
  say "    [scan] onderaan toegevoegd"
fi

if grep -q '^postcode = ""' "$CONFIG"; then
  say "    LET OP: postcode is nog leeg. Zonder postcode geeft Marktplaats geen afstand terug"
  say "            en werkt het ophaal-filter niet."
fi

"$BIN" check

# -------------------------------------------------------------------- 4. database

step "Database aanmaken en de oude bestanden overzetten"
# Deze ronde maakt het bestand aan, zaait de zoektermen uit TOML en zet seen/queue/recent over.
# Hij zoekt ook echt, dus het duurt een halve minuut.
"$BIN" run || say "    (de ronde meldde een probleem; kijk hierboven wat er misging)"

# ------------------------------------------------------------------------- 5. app

step "De app ophalen en bouwen"
if [ -d "$CHECKOUT/.git" ]; then
  git -C "$CHECKOUT" pull --ff-only
else
  git clone --depth 1 "$REPO_URL" "$CHECKOUT"
fi

cd "$CHECKOUT/app"
npm ci
npm run build
say "    gebouwd in $CHECKOUT/app/build"

# -------------------------------------------------------------------- 6. service

step "De app als gebruikersservice"
mkdir -p "$UNIT_DIR"

if [ -f "$UNIT" ]; then
  say "    $UNIT bestaat al en blijft zoals hij is"

  # Behalve twee dingen. Het configuratiepad hoort er altijd in te staan: zonder dat kan
  # het instellingenscherm een ander bestand bewerken dan de wachter leest.
  grep -q '^Environment=KAARTENJAGER_CONFIG=' "$UNIT" \
    || sed -i "/^Environment=KAARTENJAGER_DB=/a Environment=KAARTENJAGER_CONFIG=$CONFIG_DIR/kaartenjager.toml" "$UNIT"

  # En de webhook: die wil je juist later kunnen invullen, zonder de rest kwijt te raken.
  # Meegegeven betekent hier dus: zet hem, wat er ook stond.
  if [ -n "${KAARTENJAGER_DISCORD_WEBHOOK:-}" ]; then
    sed -i -e 's|^#* *Environment=KAARTENJAGER_DISCORD_WEBHOOK=.*|Environment=KAARTENJAGER_DISCORD_WEBHOOK='"$KAARTENJAGER_DISCORD_WEBHOOK"'|' "$UNIT"
    grep -q '^Environment=KAARTENJAGER_DISCORD_WEBHOOK=' "$UNIT" \
      || sed -i "/^Environment=KAARTENJAGER_DB=/a Environment=KAARTENJAGER_DISCORD_WEBHOOK=$KAARTENJAGER_DISCORD_WEBHOOK" "$UNIT"
    say "    webhook bijgewerkt"
  fi
else
  sed -e "s|%h/kaartenjager/app|$CHECKOUT/app|" \
      -e "s|^Environment=PORT=.*|Environment=PORT=$APP_PORT|" \
      -e "s|^Environment=KAARTENJAGER_CONFIG=.*|Environment=KAARTENJAGER_CONFIG=$CONFIG_DIR/kaartenjager.toml|" \
      -e "s|^Environment=TZ=.*|Environment=TZ=${KAARTENJAGER_TZ:-Europe/Amsterdam}|" \
      "$CHECKOUT/app/kaartenjager-app.service" > "$UNIT"

  if [ -n "${KAARTENJAGER_DISCORD_WEBHOOK:-}" ]; then
    sed -i "s|^# Environment=KAARTENJAGER_DISCORD_WEBHOOK=.*|Environment=KAARTENJAGER_DISCORD_WEBHOOK=$KAARTENJAGER_DISCORD_WEBHOOK|" "$UNIT"
    say "    webhook ingevuld"
  else
    say "    GEEN webhook ingevuld — de Hermes-knop zet het verzoek wel in de wachtrij,"
    say "    maar wekt Hermes niet. Zie onderaan."
  fi
  say "    geschreven naar $UNIT"
fi

if systemctl --user show-environment >/dev/null 2>&1; then
  systemctl --user daemon-reload
  systemctl --user enable kaartenjager-app
  # restart en niet `enable --now`: die laat een service die al draait ongemoeid, en dan
  # blijft de app na een update op de oude bouw staan terwijl het script zegt dat het gelukt
  # is. restart start hem ook wanneer hij nog stilstond, dus dit dekt beide gevallen.
  systemctl --user restart kaartenjager-app
  # Zonder linger stopt de service zodra je uitlogt, en dan staat de app er niet meer als je
  # er de volgende ochtend naar wilt kijken.
  loginctl enable-linger "$(id -un)" 2>/dev/null || say "    (linger aanzetten lukte niet)"
  sleep 2
  systemctl --user --no-pager --lines=0 status kaartenjager-app || true
else
  say "    Geen gebruikers-systemd bereikbaar. Start hem dan met de hand:"
  say "      cd $CHECKOUT/app && PORT=$APP_PORT node build"
fi

# ------------------------------------------------------------------ 7. wat er rest

cat <<KLAAR

==> Klaar.

De app luistert op poort $APP_PORT, bereikbaar via het tailnet-adres van deze machine.
Niet aan het open internet hangen: de app kent geen inlog, wie binnen is mag alles.

Twee dingen die dit script niet kan doen:

  1. De cronjobs bijstellen:
     - 'kaartenjager-oordeel' van 11:00 en 19:00 weghalen. Die werkte de stapel automatisch
       af, en dat is nu een knop in de app.
     - 'kaartenjager-scan' op "*/5 8-22 * * *" zetten in plaats van elk uur. Een echt koopje
       is soms binnen een half uur weg.
     - 'kaartenjager-prijzen' blijft ongewijzigd.

  2. De Discord-webhook invullen, als je hem hierboven niet hebt meegegeven. Zet in
     $UNIT de regel:

       Environment=KAARTENJAGER_DISCORD_WEBHOOK=https://discord.com/api/webhooks/...

     en daarna: systemctl --user restart kaartenjager-app

KLAAR
