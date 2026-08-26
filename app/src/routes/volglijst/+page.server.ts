import type { PageServerLoad } from './$types';
import { watching } from '$lib/server/db';
import { findingActions } from '$lib/server/actions';

const PER_PAGE = 50;

export const load: PageServerLoad = async ({ url }) => {
	const shown = Number(url.searchParams.get('toon') ?? PER_PAGE);
	const findings = watching(shown + 1, 0);
	return {
		vondsten: findings.slice(0, shown),
		erIsMeer: findings.length > shown,
		volgende: shown + PER_PAGE
	};
};

export const actions = findingActions;
