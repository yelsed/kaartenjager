import type { LayoutServerLoad } from './$types';
import { heartbeat, countInbox, openReviewCount } from '$lib/server/db';

export const load: LayoutServerLoad = async () => {
	return {
		heartbeat: heartbeat(),
		nieuw: countInbox(),
		openVerzoeken: openReviewCount()
	};
};
