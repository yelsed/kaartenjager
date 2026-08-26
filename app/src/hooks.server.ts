import type { HandleServerError } from '@sveltejs/kit';
import { ConfigurationError } from '$lib/server/db';

// Een installatieprobleem hoort te vertellen wat eraan schort. Alles wat we niet herkennen
// blijft achter een algemene melding: dat is een fout in de code, en die hoort in het log.
export const handleError: HandleServerError = ({ error }) => {
	if (error instanceof ConfigurationError) {
		return { message: error.message, herkend: true };
	}
	console.error(error);
	return { message: 'Er ging iets mis. Kijk in het log van de app.', herkend: false };
};
