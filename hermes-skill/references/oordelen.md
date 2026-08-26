# Een beoordeling afwerken

Er is geen cronjob meer die de stapel om 11:00 en 19:00 leegwerkt. In plaats daarvan drukt de
gebruiker in de app op **"Hermes laten kijken"**. Die knop zet een regel in de wachtrij en
stuurt een kort bericht in dit kanaal:

```
review gevraagd: vinted:9758884187
```

Zo'n bericht is het sein om te beginnen. Een cronjob die elke tien minuten kijkt of er iets
klaarstaat zou 144 keer per dag een aanroep kosten en vrijwel altijd niets vinden; daarom
werkt het andersom en word je geroepen.

Lees eerst `references/opstelling.md`. Daarin staat welke machine dit is en waartegen een
kaart afgezet hoort te worden. Zonder dat kun je wel zeggen dat iets goedkoop is, maar niet
of het iets toevoegt.

## Werkwijze

**1. Pak de wachtrij op.**

```bash
kaartenjager reviews take
```

Dat geeft JSON: één blok per verzoek, met `id`, `key`, de titel, de prijs, de URL, de
beschrijving, wat het programma als reden en waarschuwing noteerde, en `queue_note` — waar het
programma zelf over twijfelde.

Komt daar `[]` uit, dan ben je klaar. **Meld dan niets.** De gebruiker heeft niets gevraagd
waar geen antwoord op hoeft.

`take` pakt alles op wat openstaat, dus ook verzoeken van vóór dit bericht. Dat is de
bedoeling: één ronde werkt de hele wachtrij af.

**2. Kijk per advertentie.**

Het volledige dossier van één advertentie haal je op met de sleutel uit `key`:

```bash
kaartenjager dossier vinted:9758884187
```

Voor Marktplaats staat de volledige beschrijving daar al in. Voor Vinted staat er
`(niet opgehaald)` als het programma er deze ronde niet aan toe kwam — gebruik dan
`web_extract` op de URL om de beschrijving en de foto's te zien.

**3. Beoordeel.**

Drie soorten twijfel komen langs, elk met een eigen vraag:

| `queue_note` zegt | Wat jij uitzoekt |
|---|---|
| Geheugengrootte staat niet in de titel | Welke uitvoering is dit? Een 3060 met 8 GB is een andere, goedkopere kaart dan die met 12 GB |
| Onbekend model | Wat is dit, hoeveel videogeheugen heeft het, en wat doet het tweedehands? |
| Prijs ligt onder de bodem | Is dit oplichterij of een echte buitenkans? |
| De verkoper zegt alleen ophalen | Kan verzenden alsnog, en waar zit hij? |

De gebruiker kan de knop ook indrukken zonder dat het programma ergens over twijfelde. Dan is
`queue_note` leeg en is de vraag simpelweg: is dit het waard?

Waar je bij alle gevallen naar kijkt:

- **Foto's.** Zijn het foto's van de kaart zelf, of productplaatjes van de fabrikant? Alleen
  fabrikantfoto's is de sterkste aanwijzing voor oplichterij die er is.
- **Beschrijving.** Klinkt dit als iemand die weet wat hij verkoopt, of als een tekst die
  overal op zou kunnen slaan? Staat er iets over mining, over reparatie, over "werkte nog
  prima toen ik hem eruit haalde"?
- **Verkoper.** Hoeveel beoordelingen, hoe lang actief, verkoopt hij vaker hardware of vooral
  kleding?
- **Wat het zou toevoegen.** Past het model van 27B erin — dus 20 GB of meer? Zo niet, zeg
  dat meteen: een kaart van 16 GB is voor dit doel onbruikbaar hoe goed hij verder ook is.
  En hoe verhoudt hij zich tot de 3090 Ti van €945,70 die het ijkpunt is?
- **De prijs zelf.** Een kaart op een derde van de marktprijs is bijna nooit echt. Een kaart
  op tachtig procent kan heel goed een verhuizing zijn.

**4. Schrijf het antwoord terug.**

Per verzoek één opdracht. Het oordeel zelf gaat via stdin, want een meerregelige tekst door
een shell-argument persen loopt vast op aanhalingstekens:

```bash
kaartenjager reviews answer 12 --recommendation kijken <<'EOF'
Foto's zijn van de kaart zelf, serienummer zichtbaar, geen mining genoemd.
Verkoper heeft veertig beoordelingen en verkoopt vaker hardware.
Gaat normaal voor €320-380, dus €240 is scherp maar niet te mooi.
EOF
```

`--recommendation` kent precies drie waarden:

| Waarde | Wanneer |
|---|---|
| `kijken` | Dit is het waard. Zeg in de tekst waarom |
| `overslaan` | Niet interessant. Zeg waarom niet: verkeerde uitvoering, te weinig geheugen, gewone marktprijs |
| `oplichterij` | Dit klopt niet. Zeg wat je zag |

Kom je er niet uit — de pagina is weg, `web_extract` levert niets, de advertentie is
onleesbaar — sluit het verzoek dan af als mislukt in plaats van het te laten hangen:

```bash
kaartenjager reviews fail 12 --reason "advertentie is verwijderd, geen pagina meer"
```

Een verzoek dat je oppakt maar nooit beantwoordt komt na een uur vanzelf terug in de
wachtrij, en na drie van die rondes sluit het programma het zelf als mislukt af. Dat is een
vangnet, geen werkwijze: elke terugkeer kost opnieuw een aanroep.

**5. Meld kort terug.**

Het uitgebreide oordeel staat al in de app, onder de advertentie. In het kanaal volstaat een
paar regels:

```
Drie beoordeeld.

· Sapphire RX 6800 XT 16GB, €240 — kijken. Foto's van de kaart zelf,
  verkoper met veertig beoordelingen. Gaat normaal voor €320-380.
· "videokaart nvidia", €90 — oplichterij. Alleen fabrikantfoto's,
  verkoper zonder beoordelingen.
· RTX 3060, €130 — overslaan. Beschrijving noemt 8GB, dus de goedkope uitvoering.
```

Geen dossierblokken in het kanaal: die staan in de app, en dat was precies het punt van de
verhuizing.

## Als je een model vaker ziet

Komt hetzelfde onbekende model meerdere keren langs, noteer dat dan in je oordeel. Zondag
gebruik je die notities om er een regel voor voor te stellen, zodat het programma hem
voortaan zelf vangt.
