import type { Handle, HandleServerError } from '@sveltejs/kit';
import { text } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';
import { ConfigurationError } from '$lib/server/db';

/** Alleen deze methodes veranderen iets, en alleen die hoeven beschermd te worden. */
const CHANGING_METHODS = ['POST', 'PUT', 'PATCH', 'DELETE'];

/** Wat een browser stuurt bij een gewone formulierpost. */
const FORM_TYPES = [
	'application/x-www-form-urlencoded',
	'multipart/form-data',
	'text/plain'
];

/**
 * Weert formulierposts van een vreemde website.
 *
 * Dit vervangt de ingebouwde controle van SvelteKit, die op host én schema vergelijkt.
 * adapter-node kent zijn eigen schema niet — zonder ORIGIN gokt hij "https"
 * (`get_origin()` in files/handler.js) — terwijl de app over gewoon HTTP op het tailnet
 * draait. Daardoor week de herkomst altijd af en kreeg elke knop 403.
 *
 * Op host vergelijken houdt precies de aanval tegen waar het om gaat: een vreemde site die
 * jouw browser laat posten, bijvoorbeeld om betaalde Hermes-beoordelingen in de wachtrij te
 * zetten. En het blijft werken of je de app nu via het IP, via `openbinker` of via de
 * MagicDNS-naam opent, zonder dat er per machine iets ingevuld moet worden.
 *
 * Staat de app ooit achter een reverse proxy, zet dan de herkomst die de browser ziet in
 * KAARTENJAGER_TRUSTED_ORIGINS (komma-gescheiden).
 */
export const handle: Handle = async ({ event, resolve }) => {
	const refusal = whyRefused(event.request, event.url);
	if (refusal) {
		return text(refusal, { status: 403 });
	}
	return resolve(event);
};

function whyRefused(request: Request, url: URL): string | null {
	if (!CHANGING_METHODS.includes(request.method)) return null;

	const contentType = (request.headers.get('content-type') ?? '').split(';')[0].trim();
	if (!FORM_TYPES.includes(contentType)) return null;

	const origin = request.headers.get('origin');
	if (!origin) {
		// Browsers sturen deze kop bij formulierposts altijd mee. Ontbreekt hij, dan komt het
		// verzoek ergens anders vandaan — en dan willen we het niet blind vertrouwen.
		return 'Geweigerd: dit verzoek heeft geen Origin-kop.';
	}

	if (trustedOrigins().includes(origin)) return null;

	let host: string;
	try {
		host = new URL(origin).host;
	} catch {
		return `Geweigerd: "${origin}" is geen geldige herkomst.`;
	}

	if (host === url.host) return null;

	return (
		`Geweigerd: dit verzoek komt van ${host} en de app draait op ${url.host}. ` +
		'Open de app op hetzelfde adres, of zet deze herkomst in KAARTENJAGER_TRUSTED_ORIGINS.'
	);
}

function trustedOrigins(): string[] {
	return (env.KAARTENJAGER_TRUSTED_ORIGINS ?? '')
		.split(',')
		.map((origin) => origin.trim())
		.filter(Boolean);
}

// Een installatieprobleem hoort te vertellen wat eraan schort. Alles wat we niet herkennen
// blijft achter een algemene melding: dat is een fout in de code, en die hoort in het log.
export const handleError: HandleServerError = ({ error }) => {
	if (error instanceof ConfigurationError) {
		return { message: error.message, herkend: true };
	}
	console.error(error);
	return { message: 'Er ging iets mis. Kijk in het log van de app.', herkend: false };
};
