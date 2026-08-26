import type { LayoutServerLoad } from './$types';
import { heartbeat, countInbox, openReviewCount } from '$lib/server/db';
import { roundState } from '$lib/server/ronde';

export const load: LayoutServerLoad = async () => {
	return {
		heartbeat: heartbeat(),
		nieuw: countInbox(),
		openVerzoeken: openReviewCount(),
		ronde: roundState()
	};
};
