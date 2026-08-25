# De machine en waar het voor is

Zonder dit kan er geen antwoord komen op "wat zou deze kaart toevoegen?". Het programma
rekent al met de `[system]`-regels uit de configuratie, maar jij hebt ook nodig waaróm die
getallen zo staan.

## De machine

| | |
|---|---|
| Processor | AMD Ryzen 7 5800X, 8 kernen |
| Werkgeheugen | 31 GB DDR4 in twee kanalen, ongeveer 48 GB/s |
| Videokaart nu | NVIDIA RTX 3070 Ti, **8 GB** |
| Voeding | Corsair RM850 |
| Moederbord | Gigabyte X570 AORUS ELITE |
| Kast | Corsair 4000D Airflow, 7 sleuven, kaart tot 360 mm |
| Besturingssysteem | Arch Linux (omarchy), ollama, oh-my-pi |

De vier PCIe-sleuven: `PCIEX16` (x16), `PCIEX4` (x4), en twee maal x1.

## Waar het voor is

Het draaien van **qwen3.8 27B** op Q4_K_M, lokaal, als codeermodel achter oh-my-pi. Dat model
is ongeveer even sterk als Claude Sonnet 5 op programmeertaken, en dat is opmerkelijk voor
27 miljard parameters.

Het probleem: **8 GB is te weinig.** Het model heeft nodig:

| | |
|---|---|
| Gewichten op Q4_K_M | 16,0 GB |
| KV-cache bij 65k context | 2,2 GB |
| Rekenbuffers | 0,6 GB |
| Bureaublad | 0,7–1,5 GB |
| **Samen** | **ongeveer 19,5 GB** |

Daarvan past er nu 16 lagen van de 65 op de kaart; de rest staat in het werkgeheugen. Gemeten:

| Situatie | Genereren |
|---|---|
| Lege context | 7,21 tok/s |
| 20k tokens | 3,04 tok/s |
| 47k tokens | 1,72 tok/s |
| **60k tokens (wat omp echt stuurt)** | **~1,4 tok/s** |

Een volle beurt van 60k prompt plus 2.800 tokens antwoord duurt ongeveer 35 minuten.

## Waarom videogeheugen alles bepaalt

Het genereren van één token leest **alle** gewichten van het model. Bij 16 GB aan gewichten
en dertig tokens per seconde is dat 480 GB per seconde aan geheugenverkeer. Staan die
gewichten in het werkgeheugen, dan is de bovengrens 48 GB/s gedeeld door 16 GB, oftewel drie
tokens per seconde — en dat is de hele verklaring.

Daarom telt bij het beoordelen van een kaart deze volgorde:

1. **Past het model erin?** Onder de 20 GB is het antwoord nee, en dan doet de rest er niet
   toe. Een kaart van 16 GB is voor dit doel onbruikbaar, hoe snel hij verder ook is.
2. **Hoeveel bandbreedte?** Dat is recht evenredig met tokens per seconde. Bandbreedte gedeeld
   door de omvang van de gewichten is de bovengrens.
3. **Hoeveel rekenkracht?** Telt alleen bij het verwerken van de prompt, niet bij het
   genereren. Omp stuurt prompts van 60.000 tokens, dus het telt hier meer dan gebruikelijk.

## Het plan waar dit tegen afgezet wordt

Een tweedehands **RTX 3090 Ti voor €945,70**, gevonden op Vinted. 24 GB, 1008 GB/s, 450 W.
Verwacht ongeveer 30 tot 32 tokens per seconde op qwen3.8 27B — ruim twintig keer wat er nu
uit komt.

De 3070 Ti van 8 GB blijft ernaast, als tweede kaart voor grotere modellen of om buiten de
kast aan een riserkabel te hangen.

Beoordeel elke kaart die langskomt **tegen dit plan**, niet in het luchtledige. De vraag is
niet "is dit een goede kaart" maar "is dit beter dan een 3090 Ti voor €945,70, en waarom".

## Vergelijkingstabel

Voor kaarten die vaak langskomen, met wat ze op dit model zouden doen:

| Kaart | VRAM | Bandbreedte | Past 27B? | Verwacht | Tensor |
|---|---|---|---|---|---|
| RTX 3070 Ti (huidig) | 8 GB | 608 GB/s | nee | 1,4 tok/s | 87 TFLOPS |
| RTX 3080 10GB | 10 GB | 760 GB/s | nee | — | 119 TFLOPS |
| RTX 4070 | 12 GB | 504 GB/s | nee | — | 116 TFLOPS |
| RTX 4060 Ti 16GB | 16 GB | 288 GB/s | **nee** | — | 88 TFLOPS |
| RTX 4080 / 4080 Super | 16 GB | 717–736 GB/s | **nee** | — | 195 TFLOPS |
| **RTX 3090** | 24 GB | 936 GB/s | ja | ~28 tok/s | 142 TFLOPS |
| **RTX 3090 Ti** | 24 GB | 1008 GB/s | ja | ~30 tok/s | 160 TFLOPS |
| **RTX 4090** | 24 GB | 1008 GB/s | ja | ~30 tok/s | **330 TFLOPS** |
| RX 7900 XTX | 24 GB | 960 GB/s | ja | ~25 tok/s | ROCm, rommeliger |
| Tesla P40 | 24 GB | 347 GB/s | ja | ~10 tok/s | geen koeling, oud |
| RTX 5090 | 32 GB | 1792 GB/s | ruim | ~55 tok/s | 450 TFLOPS |

**Let op de 16 GB-rij.** Een 4080 Super is een prachtige kaart en volstrekt ongeschikt voor dit
doel, want 16 min 3 GB overhead laat ruimte voor ongeveer 22 miljard parameters en het model
heeft er 27.

**Let op de 4090 tegenover de 3090 Ti.** Zelfde geheugen, zelfde bandbreedte, dus **dezelfde
snelheid bij het genereren**. Het verschil zit in de tensorrekenkracht — twee keer zoveel, dus
ongeveer twee keer zo snel bij het verwerken van de prompt — en in twee dingen die niets met
taalmodellen te maken hebben:

- **DLSS 3 Frame Generation** werkt alleen op Ada (40-serie en nieuwer), omdat het de Optical
  Flow Accelerator nodig heeft. Op Linux is dat de enige werkende manier: de truc met een
  tweede kaart via Lossless Scaling werkt daar niet, want de Vulkan-laag krijgt de swapchain
  niet verplaatst.
- **60 tot 70 procent meer prestaties in spellen**, en meer bij raytracing.

## Wat er verder aan de opstelling hangt

De 3090 Ti past niet zomaar naast de 3070 Ti in de kast: een kaart van drie sleuven dekt
`PCIEX4` af. Er zijn twee routes, en welke het wordt hangt aan één meting.

| | Route A | Route B |
|---|---|---|
| Opstelling | beide kaarten in de kast | 3070 Ti aan een riser op een rek ernaast |
| Voeding | RM1200x, €219 nieuw | RM850 blijft, plus €70 tweedehands op het rek |
| Totaal | €1.164,70 | €1.068–1.088 |
| Ruimte voor kaart drie | nee | ja |

Voedingen van 700 W of meer en PCIe 4.0 x16-riserkabels zijn dus ook echt nodig, niet
opportunistisch. Een riser van €14,99 tegenover €25–45 winkelprijs is een echte vondst.

## Wat een goed antwoord bevat

Als er gevraagd wordt wat een kaart zou toevoegen:

1. **Past het model erin?** Zo niet, zeg dat meteen en houd op met rekenen.
2. **Hoeveel tokens per seconde**, geschat uit bandbreedte gedeeld door 16 GB aan gewichten,
   maal ongeveer 0,5 voor de praktijk.
3. **Wat het kost tegenover de 3090 Ti van €945,70**, en wat je voor dat verschil krijgt.
4. **Wat het níét toevoegt.** Meer bandbreedte bij hetzelfde geheugen is winst; meer
   rekenkracht bij dezelfde bandbreedte verandert niets aan het genereren.
5. **Past hij in de kast en op de voeding?** 450 W plus 155 W overig verbruik op een RM850 is
   krap maar haalbaar voor één kaart.
