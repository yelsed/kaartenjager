import type { PageServerLoad } from './$types';
import type { Actions } from '@sveltejs/kit';
import { fail } from '@sveltejs/kit';
import { leesInstellingen, bewaarInstellingen } from '$lib/server/instellingen';
import { startRound } from '$lib/server/ronde';

export const load: PageServerLoad = async () => {
	return await leesInstellingen();
};

export const actions: Actions = {
	nuZoeken: async () => {
		const { started, message } = startRound();
		return { success: started, message };
	},

	bewaren: async ({ request }) => {
		const form = await request.formData();
		const inhoud = form.get('inhoud');
		if (typeof inhoud !== 'string') {
			return fail(400, { success: false, message: 'Geen inhoud meegestuurd.', controle: '' });
		}

		const uitkomst = await bewaarInstellingen(inhoud);
		// De tekst gaat altijd mee terug. Bij een afkeuring moet hij blijven staan zoals hij
		// getypt is: hem terugzetten naar de bewaarde versie zou het werk weggooien juist op
		// het moment dat er nog iets te herstellen valt.
		return uitkomst.success
			? { ...uitkomst, inhoud }
			: fail(422, { ...uitkomst, inhoud });
	}
};
