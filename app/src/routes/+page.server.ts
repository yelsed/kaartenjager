import type { PageServerLoad } from './$types';
import { inbox, products } from '$lib/server/db';
import { findingActions } from '$lib/server/actions';

const PER_PAGE = 50;

export const load: PageServerLoad = async ({ url }) => {
	// Na een half jaar staan er duizenden advertenties in; een pagina die ze allemaal ophaalt
	// wordt traag zonder dat iemand er iets aan heeft.
	const shown = Number(url.searchParams.get('toon') ?? PER_PAGE);
	const allProducts = products();
	const requested = url.searchParams.get('product');
	const selected = allProducts.some((product) => product.id === requested && product.enabled)
		? requested
		: null;
	const findings = inbox(shown + 1, 0, selected);
	return {
		producten: allProducts,
		gekozenProduct: selected,
		vondsten: findings.slice(0, shown),
		erIsMeer: findings.length > shown,
		volgende: shown + PER_PAGE
	};
};

export const actions = findingActions;
