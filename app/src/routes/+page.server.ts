import type { PageServerLoad } from './$types';
import { inbox } from '$lib/server/db';
import { findingActions } from '$lib/server/actions';

const PER_PAGE = 50;

export const load: PageServerLoad = async ({ url }) => {
	// Na een half jaar staan er duizenden advertenties in; een pagina die ze allemaal ophaalt
	// wordt traag zonder dat iemand er iets aan heeft.
	const shown = Number(url.searchParams.get('toon') ?? PER_PAGE);
	const findings = inbox(shown + 1, 0);
	return {
		vondsten: findings.slice(0, shown),
		erIsMeer: findings.length > shown,
		volgende: shown + PER_PAGE
	};
};

export const actions = findingActions;
