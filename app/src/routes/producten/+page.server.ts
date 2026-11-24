import type { PageServerLoad } from './$types';
import { products } from '$lib/server/db';
import { productActions } from '$lib/server/actions';

export const load: PageServerLoad = async () => ({ producten: products() });

export const actions = productActions;
