import type { PageServerLoad } from './$types';
import { inbox, inboxTotal, products, type InboxFilters } from '$lib/server/db';
import { findingActions } from '$lib/server/actions';

const PER_PAGE = 50;

function positiveNumber(raw: string | null): number | null {
	if (raw === null || raw.trim() === '') return null;
	const value = Number(raw);
	return Number.isFinite(value) && value >= 0 ? value : null;
}

function filtersFrom(parameters: URLSearchParams): InboxFilters {
	const level = parameters.get('trap');
	return {
		minimumPrice: positiveNumber(parameters.get('prijsVanaf')),
		maximumPrice: positiveNumber(parameters.get('prijsTot')),
		minimumScorePerEuro: positiveNumber(parameters.get('scoreVanaf')),
		minimumMemoryGb: positiveNumber(parameters.get('geheugenVanaf')),
		level: level === 'ultra' || level === 'top' ? level : null
	};
}

export const load: PageServerLoad = async ({ url }) => {
	// Na een half jaar staan er duizenden advertenties in; een pagina die ze allemaal ophaalt
	// wordt traag zonder dat iemand er iets aan heeft.
	const shown = Number(url.searchParams.get('toon') ?? PER_PAGE);
	const allProducts = products();
	const requested = url.searchParams.get('product');
	const selected = allProducts.some((product) => product.id === requested && product.enabled)
		? requested
		: null;
	const filters = filtersFrom(url.searchParams);
	const findings = inbox(shown + 1, 0, selected, filters);
	return {
		producten: allProducts,
		totaal: inboxTotal(),
		gekozenProduct: selected,
		filters,
		vondsten: findings.slice(0, shown),
		erIsMeer: findings.length > shown,
		volgende: shown + PER_PAGE
	};
};

export const actions = findingActions;
