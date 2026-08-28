// Het configuratiebestand bewerken vanuit de app.
//
// Tot nu toe was dit bewust niet mogelijk: een fout in de drempels of de kaartregels moest
// niet met één klik te maken zijn. Maar de helft die wél in de app stond — de zoektermen —
// is precies de helft die niets bepaalt. Wie een regel voor een nieuw kaartmodel wilde, of
// een voeding van 1000 W in plaats van 700 W, moest alsnog het bestand op de server open
// zien te krijgen. Twee lijsten die bij elkaar horen op twee plekken, waarvan er één
// onbereikbaar is, levert vooral verwarring op.
//
// De angst blijft terecht, dus die is opgelost in plaats van weggewuifd:
//
//   1. Er wordt niets weggeschreven wat `kaartenjager check` niet goedkeurt. Dat is
//      dezelfde controle die de wachter zelf doet, uitgevoerd door hetzelfde programma —
//      geen nabouw in TypeScript die er langzaam vanaf gaat wijken.
//   2. De vorige versie blijft naast het bestand staan, dus terugdraaien kan altijd.
//
// Daardoor is de ergste uitkomst een afgekeurde bewaarpoging met de reden erbij, in plaats
// van een wachter die vannacht stilvalt.

import { execFile } from 'node:child_process';
import { copyFile, readFile, rename, unlink, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import { env } from '$env/dynamic/private';

const run = promisify(execFile);

/** Ruim voor een controle die alleen leest en rekent; een ronde valt hier niet onder. */
const CONTROLE_SECONDEN = 30;

export type Instellingen = {
	pad: string;
	inhoud: string;
	/** Wat `kaartenjager check` van de huidige inhoud vindt. */
	controle: string;
	/** Waar staat wat, zodat het bewerkscherm niet hoeft te raden. */
	probleem: string | null;
};

export type BewaarUitkomst = {
	success: boolean;
	message: string;
	/** De uitvoer van de controle, geslaagd of niet. Dit is waar de fout in staat. */
	controle: string;
};

function binary(): string {
	return env.KAARTENJAGER_BIN ?? join(homedir(), '.local/bin/kaartenjager');
}

/**
 * Het pad opvragen aan het programma zelf. De zoekvolgorde hier nabouwen zou werken tot
 * iemand hem in Rust verandert, en een app die een ánder bestand bewerkt dan de wachter
 * leest is erger dan geen bewerkscherm: je wijziging lijkt dan te lukken en doet niets.
 */
async function configPath(): Promise<string> {
	const { stdout } = await run(binary(), ['config', 'path'], {
		timeout: CONTROLE_SECONDEN * 1000
	});
	const pad = stdout.trim();
	if (!pad) throw new Error('`kaartenjager config path` gaf niets terug.');
	return pad;
}

/**
 * Draait de controle en geeft terug wat hij zei. Een afkeuring is hier geen uitzondering
 * maar een antwoord: het bestand kán stuk zijn, en dan is de reden precies wat het scherm
 * moet tonen.
 */
async function controleer(pad: string): Promise<{ ok: boolean; uitvoer: string }> {
	try {
		const { stdout, stderr } = await run(binary(), ['check', '--config', pad], {
			timeout: CONTROLE_SECONDEN * 1000
		});
		return { ok: true, uitvoer: [stdout, stderr].join('').trim() };
	} catch (error) {
		const detail = error as { stdout?: string; stderr?: string; message?: string };
		const uitvoer = [detail.stdout ?? '', detail.stderr ?? ''].join('').trim();
		return { ok: false, uitvoer: uitvoer || (detail.message ?? 'De controle liep vast.') };
	}
}

export async function leesInstellingen(): Promise<Instellingen> {
	let pad: string;
	try {
		pad = await configPath();
	} catch (error) {
		return {
			pad: '',
			inhoud: '',
			controle: '',
			probleem:
				'Het configuratiebestand is niet te vinden. ' +
				`Klopt KAARTENJAGER_BIN (${binary()}), en staat er een kaartenjager.toml op de server? ` +
				String(error)
		};
	}

	try {
		const inhoud = await readFile(pad, 'utf8');
		const { uitvoer } = await controleer(pad);
		return { pad, inhoud, controle: uitvoer, probleem: null };
	} catch (error) {
		return {
			pad,
			inhoud: '',
			controle: '',
			probleem: `${pad} is niet te lezen: ${String(error)}`
		};
	}
}

/**
 * Bewaart alleen wat de controle goedkeurt.
 *
 * De nieuwe versie wordt eerst náást het echte bestand gezet, in dezelfde map. Dat is geen
 * detail: `cards.auto.toml` van de wekelijkse prijsherziening wordt gezocht naast het
 * configuratiebestand, dus een controle vanuit /tmp zou een andere samenstelling keuren dan
 * er straks draait.
 */
export async function bewaarInstellingen(inhoud: string): Promise<BewaarUitkomst> {
	let pad: string;
	try {
		pad = await configPath();
	} catch (error) {
		return { success: false, message: `Het bestand is niet te vinden: ${error}`, controle: '' };
	}

	const kandidaat = `${pad}.nieuw`;
	try {
		await writeFile(kandidaat, inhoud, 'utf8');
	} catch (error) {
		return {
			success: false,
			message: `Er kon niet geschreven worden naast ${pad}: ${error}`,
			controle: ''
		};
	}

	const { ok, uitvoer } = await controleer(kandidaat);
	if (!ok) {
		await unlink(kandidaat).catch(() => {});
		return {
			success: false,
			message: 'Niet bewaard — de controle keurde dit af. Er is niets veranderd.',
			controle: uitvoer
		};
	}

	try {
		// De vorige versie blijft staan. Keurt de controle iets goed dat tóch niet is wat je
		// bedoelde, dan is dit de weg terug.
		await copyFile(pad, `${pad}.vorige`);
		await rename(kandidaat, pad);
	} catch (error) {
		await unlink(kandidaat).catch(() => {});
		return { success: false, message: `Bewaren mislukte: ${error}`, controle: uitvoer };
	}

	return {
		success: true,
		message: `Bewaard. De vorige versie staat in ${pad}.vorige. De eerstvolgende ronde gebruikt dit.`,
		controle: uitvoer
	};
}
