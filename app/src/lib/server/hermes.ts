// Het wekbericht naar Hermes.
//
// De wachtrij in de database is de waarheid; dit bericht is alleen het belletje. Er is geen
// poll-cron, want die zou 144 agent-aanroepen per dag kosten voor een wachtrij die vrijwel
// altijd leeg is. Gaat het bericht verloren, dan blijft het verzoek gewoon staan en meldt de
// uurlijkse scan het na een uur alsnog in Discord.

import { env } from '$env/dynamic/private';

export type WakeResult = { sent: boolean; reason?: string };

export async function wakeHermes(key: string): Promise<WakeResult> {
	const webhook = env.KAARTENJAGER_DISCORD_WEBHOOK;
	if (!webhook) {
		return {
			sent: false,
			reason:
				'Geen KAARTENJAGER_DISCORD_WEBHOOK ingesteld, dus Hermes is niet gewekt. ' +
				'Het verzoek staat wel in de wachtrij.'
		};
	}

	try {
		const response = await fetch(webhook, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(wakeMessage(key, env.KAARTENJAGER_HERMES_BOT_ID)),
			signal: AbortSignal.timeout(5000)
		});
		if (!response.ok) {
			return { sent: false, reason: `Discord gaf HTTP ${response.status} terug.` };
		}
		return { sent: true };
	} catch (error) {
		// Het verzoek staat al in de wachtrij, dus dit is geen reden om de klik te laten falen.
		return { sent: false, reason: `Wekbericht mislukte: ${(error as Error).message}` };
	}
}

// Hermes negeert berichten van webhooks, tenzij ze hem noemen (DISCORD_ALLOW_BOTS=mentions).
// Zonder bot-ID blijft het een bericht waar de gebruiker zelf op moet reageren.
export function wakeMessage(key: string, botId: string | undefined) {
	const mention = botId?.trim();
	if (!mention) {
		return { content: `review gevraagd: ${key}` };
	}
	return {
		content: `<@${mention}> review gevraagd: ${key}`,
		allowed_mentions: { users: [mention] }
	};
}
