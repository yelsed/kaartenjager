// Zelf een ronde starten, vanuit de app.
//
// Normaal doet de cronjob dit elk uur. Deze knop is voor als je niet wilt wachten: net een
// zoekterm toegevoegd, of je wilt gewoon weten of er nu iets staat.

import { execFile } from 'node:child_process';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { env } from '$env/dynamic/private';

/** Een ronde duurt met de voorbeeldconfiguratie ruim een minuut. Hierboven gaat er iets mis. */
const MAX_RUN_SECONDS = 600;

export type RoundState = {
	running: boolean;
	startedAt: number | null;
	/** De uitkomst van de laatste ronde die via deze knop liep. */
	lastResult: string | null;
};

// Eén proces, dus een variabele volstaat om te weten of er al iets loopt. Gaat de app
// halverwege onderuit, dan is die kennis weg — vandaar dat MAX_RUN_SECONDS de deur ook
// vanzelf weer opent.
let startedAt: number | null = null;
let lastResult: string | null = null;

function nowSeconds(): number {
	return Math.floor(Date.now() / 1000);
}

function binary(): string {
	return env.KAARTENJAGER_BIN ?? join(homedir(), '.local/bin/kaartenjager');
}

export function roundState(): RoundState {
	if (startedAt !== null && nowSeconds() - startedAt > MAX_RUN_SECONDS) {
		startedAt = null;
		lastResult = 'De ronde duurde te lang en is losgelaten.';
	}
	return { running: startedAt !== null, startedAt, lastResult };
}

/**
 * Start een ronde en wacht er niet op. Een ronde duurt ruim een minuut; daar een
 * formulierpost op laten wachten levert alleen een pagina op die lijkt vast te zitten. De
 * hartslag bovenaan laat vanzelf zien wanneer hij klaar is.
 *
 * Eén tegelijk. Twee rondes naast elkaar leveren niets extra's op en verdubbelen wel het
 * aantal verzoeken aan Vinted en Marktplaats, wat precies de blokkade oplevert die het
 * verzoekbudget moet voorkomen.
 */
export function startRound(): { started: boolean; message: string } {
	const state = roundState();
	if (state.running) {
		return { started: false, message: 'Er loopt al een ronde. Even geduld.' };
	}

	startedAt = nowSeconds();
	lastResult = null;

	execFile(
		binary(),
		['run'],
		{ timeout: MAX_RUN_SECONDS * 1000, maxBuffer: 4 * 1024 * 1024 },
		(error, stdout, stderr) => {
			startedAt = null;

			if (error) {
				const detail = stderr.trim().split('\n').slice(-3).join(' · ');
				lastResult = `De ronde mislukte: ${detail || error.message}`;
				console.error('kaartenjager run mislukte', error, stderr);
				return;
			}

			const outliers = stdout.trim();
			lastResult = outliers
				? 'De ronde vond een uitschieter; die staat hieronder en is naar Discord gestuurd.'
				: 'De ronde is klaar. Geen uitschieters.';

			// Wat het programma op stdout zet, gaat bij de cronjob naar Discord. Start jij de
			// ronde vanuit de app, dan is er geen cronjob die dat doet — en zonder deze regel
			// zou de melding verloren gaan terwijl `pushed_at` wél gestempeld is, zodat je hem
			// ook later nooit meer krijgt.
			if (outliers) void forwardToDiscord(outliers);
		}
	);

	return { started: true, message: 'Ronde gestart. Dit duurt ongeveer een minuut.' };
}

async function forwardToDiscord(message: string): Promise<void> {
	const webhook = env.KAARTENJAGER_DISCORD_WEBHOOK;
	if (!webhook) return;

	try {
		await fetch(webhook, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			// Discord kapt boven de tweeduizend tekens af met een fout in plaats van te knippen.
			body: JSON.stringify({ content: message.slice(0, 1900) }),
			signal: AbortSignal.timeout(5000)
		});
	} catch (error) {
		console.error('uitschieter niet naar Discord gestuurd', error);
	}
}
