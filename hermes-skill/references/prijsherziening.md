# De prijstabel herzien

Draait zondag om 09:00. Doel: voorkomen dat de tabel stilletjes veroudert.

Tweedehandsprijzen zakken continu. Een drempel die in augustus scherp was, is in december
gewoon de marktprijs — en dan zwijgt de wachter zonder dat iemand het merkt. Dat is de
gevaarlijkste storing die dit ding kent, want stil falen ziet er precies zo uit als "er is
niets te koop".

## Werkwijze

**1. Kijk wat er nu staat.**

```bash
kaartenjager check
cat ~/.config/kaartenjager/kaartenjager.toml
cat ~/.config/kaartenjager/cards.auto.toml   # kan ontbreken
```

Regels in `kaartenjager.toml` zijn met de hand gezet en winnen altijd. Voorstel je daar iets
voor, dan wordt het toegepast noch gemeld — het bestand van de gebruiker is de baas. Kijk dus
vooral naar modellen die daar níét in staan.

**2. Zoek per model uit wat de markt doet.**

Gebruik `web_search` en waar nodig `web_extract`. Kijk naar wat er nú te koop staat op
Marktplaats, Vinted en Tweakers V&A. Tel hoeveel advertenties je hebt gezien en noteer de
mediaan — dat getal komt in je rapport, zodat de gebruiker kan zien of een voorstel op iets
gebaseerd is of op drie toevallige aanbiedingen.

Onder de tien advertenties is de steekproef te klein. Stel dan niets voor en zeg dat erbij.

**3. Schrijf een voorstel.**

Naar `/tmp/cards.proposed.toml`, in dezelfde vorm als de tabel:

```toml
[[card]]
name = "RTX 3080"
patterns = ["3080"]
exclude_patterns = ["3080 12gb", "3080 ti"]
vram_gb = 10
bandwidth_gbs = 760
tdp_watt = 320
used_price_low = 270
used_price_high = 360
alert_below = 240
suspicious_below = 150
source = "22 advertenties Marktplaats en Vinted, mediaan 315"
```

Regels waar niets aan verandert laat je weg. Ze blijven vanzelf staan.

**Nieuwe modellen hebben `source` nodig** met waar de prijs vandaan komt. Zonder bruikbare
bronvermelding wordt de regel geweigerd, en terecht: er is anders geen manier om een
opgezocht getal van een verzonnen getal te onderscheiden.

**4. Laat het keuren.**

```bash
kaartenjager config apply --from /tmp/cards.proposed.toml --check   # eerst kijken
kaartenjager config apply --from /tmp/cards.proposed.toml           # daarna toepassen
```

Het programma controleert:

| Controle | Regel |
|---|---|
| Syntax en velden | moet als TOML laden, alles aanwezig en van het juiste type |
| Innerlijke logica | `suspicious_below` < `alert_below` ≤ `used_price_low` ≤ `used_price_high` |
| Stapgrootte | geen waarde mag meer dan 20% verschuiven ten opzichte van nu |
| Absolute grenzen | geen bedrag onder €20 of boven €5.000 |
| Herkomst | een nieuw model heeft een bruikbare `source` nodig |

Wordt iets geweigerd, **probeer het dan niet te omzeilen**. Die grenzen bestaan om precies
één ding te voorkomen: dat een fout van jou de wachter uitschakelt zonder dat iemand het
merkt. Zet de weigering in je rapport en laat de gebruiker beslissen.

De 20%-grens betekent dat een echte instorting twee weken kost om bij te trekken. Dat is
bewust zo.

**5. Rapporteer.**

Neem letterlijk over wat `config apply` afdrukte — dat bevat de wijzigingen, de percentages
en de weigeringen al. Vul aan met waaróm iets veranderde:

```
· RTX 3080 10GB   €300–400 → €275–365   -10%
  22 advertenties, mediaan €315. Aanbod loopt op sinds de 50-serie er is.
```

En zet er onder wat er níét veranderde. Zonder die regel kan de gebruiker niet zien of je
zweeg omdat er niets veranderde of omdat je vastliep.

Is er niets te melden, dan is drie regels genoeg:

```
Prijstabel — geen wijzigingen. 11 modellen bekeken, alles binnen 3%.
```

## Wat je niet doet

**Niet rechtstreeks in de configuratie schrijven.** Altijd via `config apply`.

**Geen modellen weghalen.** Een model dat je even niet tegenkomt hoort te blijven staan; het
komt volgende maand weer langs.

**Geen drempels omhoog gooien omdat er weinig gemeld wordt.** Weinig meldingen betekent
meestal dat er weinig koopjes zijn, niet dat de drempels te streng staan. Verruim alleen als
je in de markt ziet dat de prijzen echt gedaald zijn.
