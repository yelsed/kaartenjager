import type { PageServerLoad } from './$types';
import { searchTerms, enabledTermCount, maxEnabledTerms, products } from '$lib/server/db';
import { termActions } from '$lib/server/actions';

export const load: PageServerLoad = async () => {
	return {
		termen: searchTerms(),
		producten: products(),
		aan: enabledTermCount(),
		grens: maxEnabledTerms()
	};
};

export const actions = termActions;
