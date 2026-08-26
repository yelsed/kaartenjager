// De handelingen die op elk tabblad werken. Elk +page.server.ts hangt deze lijst op, zodat
// archiveren vanuit het archief hetzelfde doet als archiveren vanuit de inbox.

import { fail } from '@sveltejs/kit';
import type { Actions } from '@sveltejs/kit';
import * as store from './db';
import { wakeHermes } from './hermes';
import { startRound } from './ronde';

/// Op elk tabblad beschikbaar, zodat de knop in de kop vanaf elke pagina werkt.
const runActions: Actions = {
	nuZoeken: async () => {
		const { started, message } = startRound();
		return { success: started, message };
	}
};

function requireKey(form: FormData): string | null {
	const key = form.get('key');
	return typeof key === 'string' && key.length > 0 ? key : null;
}

export const findingActions: Actions = {
	...runActions,

	archiveren: async ({ request }) => {
		const form = await request.formData();
		const key = requireKey(form);
		if (!key) return fail(400, { success: false, message: 'Geen advertentie meegegeven.' });
		store.setState(key, 'archived');
		return { success: true, message: 'Weggelegd. Zakt de prijs meer dan tien procent, dan komt hij terug.' };
	},

	volgen: async ({ request }) => {
		const form = await request.formData();
		const key = requireKey(form);
		if (!key) return fail(400, { success: false, message: 'Geen advertentie meegegeven.' });
		store.setState(key, 'watching');
		return { success: true, message: 'Op de volglijst gezet.' };
	},

	terug: async ({ request }) => {
		const form = await request.formData();
		const key = requireKey(form);
		if (!key) return fail(400, { success: false, message: 'Geen advertentie meegegeven.' });
		store.setState(key, 'inbox');
		return { success: true, message: 'Terug in de inbox.' };
	},

	hermes: async ({ request }) => {
		const form = await request.formData();
		const key = requireKey(form);
		if (!key) return fail(400, { success: false, message: 'Geen advertentie meegegeven.' });

		const { alreadyOpen } = store.requestReview(key);
		if (alreadyOpen) {
			return { success: true, message: 'Er stond al een verzoek open voor deze advertentie. Eentje is genoeg.' };
		}

		// De wachtrij is de waarheid; het bericht is het belletje. Mislukt het bericht, dan is
		// dat geen reden om de klik te laten falen — het verzoek staat er.
		const woken = await wakeHermes(key);
		return {
			success: true,
			message: woken.sent
				? 'Hermes is gevraagd ernaar te kijken. Het antwoord komt hieronder te staan.'
				: `Het verzoek staat in de wachtrij. ${woken.reason}`
		};
	},

	allesGezien: async () => {
		store.markEverythingSeen();
		return { success: true, message: 'Alles gezien.' };
	},

	toonWeerAlsNieuw: async () => {
		store.undoEverythingSeen();
		return { success: true, message: 'Teruggedraaid.' };
	}
};

export const termActions: Actions = {
	...runActions,

	toevoegen: async ({ request }) => {
		const form = await request.formData();
		const term = form.get('term');
		const kind = form.get('kind');
		if (typeof term !== 'string' || (kind !== 'card' && kind !== 'part')) {
			return fail(400, { success: false, message: 'Vul een zoekterm in en kies waar hij op slaat.' });
		}
		const refused = store.addTerm(term, kind);
		if (refused) return fail(400, { success: false, message: refused });
		return { success: true, message: `"${term.trim().toLowerCase()}" staat erbij.` };
	},

	aanzetten: async ({ request }) => {
		const form = await request.formData();
		const term = form.get('term');
		const enabled = form.get('enabled') === 'ja';
		if (typeof term !== 'string') return fail(400, { success: false, message: 'Geen zoekterm meegegeven.' });
		const refused = store.toggleTerm(term, enabled);
		if (refused) return fail(400, { success: false, message: refused });
		return { success: true, message: enabled ? `"${term}" staat weer aan.` : `"${term}" staat uit.` };
	},

	verwijderen: async ({ request }) => {
		const form = await request.formData();
		const term = form.get('term');
		if (typeof term !== 'string') return fail(400, { success: false, message: 'Geen zoekterm meegegeven.' });
		store.removeTerm(term);
		return { success: true, message: `"${term}" is weg.` };
	}
};
