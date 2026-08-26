import type { PageServerLoad } from './$types';
import { searchTerms, enabledTermCount, maxEnabledTerms } from '$lib/server/db';
import { termActions } from '$lib/server/actions';

export const load: PageServerLoad = async () => {
	return {
		termen: searchTerms(),
		aan: enabledTermCount(),
		grens: maxEnabledTerms()
	};
};

export const actions = termActions;
