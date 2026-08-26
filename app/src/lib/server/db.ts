// De gedeelde database. Het Rust-programma schrijft elke ronde, Hermes werkt de wachtrij af,
// en deze app leest en schrijft wat jij aanklikt — alle drie op hetzelfde bestand.
//
// Er is geen ORM en geen migratie aan deze kant: het schema hoort bij het programma, en deze
// app weigert bij een versie die hij niet kent.

import { DatabaseSync } from 'node:sqlite';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { env } from '$env/dynamic/private';

/** Het schema waar deze app op gebouwd is. Zie PRAGMA user_version in het programma. */
const SCHEMA_VERSION = 1;

/** Onder deze fractie van de archiefprijs komt een advertentie terug in de inbox. */
const RETURN_FROM_ARCHIVE_AT = 0.9;

/** Zo lang telt een vondst als nieuw zonder dat je op "alles gezien" drukt. */
const STAYS_NEW_SECONDS = 48 * 3600;

/** Boven deze ouderdom is de wachter stil gevallen in plaats van dat de markt rustig is. */
const HEARTBEAT_STALE_SECONDS = 2 * 3600;

const DAY_STARTS_AT = 8;
const DAY_ENDS_AT = 22;

export type Kind = 'card' | 'part' | 'unknown';
export type State = 'inbox' | 'archived' | 'watching';

export type Finding = {
	key: string;
	title: string;
	url: string;
	source: string;
	delivery: string;
	location: string;
	seller: string;
	condition: string;
	photoCount: number;
	description: string;
	matchedAs: string;
	kind: Kind;
	needsReview: boolean;
	percentUnderMarket: number | null;
	eurosUnderMarket: number | null;
	reasons: string[];
	warnings: string[];
	queueNote: string | null;
	becameAFindAt: number;
	judgedAt: number;
	stillAFind: boolean;
	goneSince: number | null;
	state: State;
	priceEuros: number;
	askingEuros: number;
	priceWhenArchived: number | null;
	isNew: boolean;
	/** Waar de prijs vandaan komt en waar hij nu staat, als er iets veranderd is. */
	priceMove: PriceMove | null;
	review: Review | null;
};

export type PriceMove = {
	fromEuros: number;
	toEuros: number;
	days: number;
};

export type Review = {
	id: number;
	requestedAt: number;
	answeredAt: number | null;
	verdict: string | null;
	recommendation: string | null;
	failedReason: string | null;
};

export type SearchTerm = {
	term: string;
	kind: 'card' | 'part';
	enabled: boolean;
	addedAt: number;
	addedBy: string;
};

export type Heartbeat = {
	lastRoundAt: number | null;
	problems: string[];
	stale: boolean;
	withinDayWindow: boolean;
};

/**
 * Een probleem met de installatie, niet met de code: het pad klopt niet, de database bestaat
 * nog niet, of het schema is een ander. Zulke fouten mogen wél getoond worden — anders sta je
 * bij het uitrollen naar "Internal Error" te kijken zonder te weten wat je moet doen.
 */
export class ConfigurationError extends Error {}

let connection: DatabaseSync | null = null;

export function databasePath(): string {
	return env.KAARTENJAGER_DB ?? join(homedir(), '.local/share/kaartenjager/kaartenjager.db');
}

/**
 * Eén verbinding voor het hele proces. `busy_timeout` is niet optioneel: zonder die regel
 * geeft een klik die samenvalt met het wegschrijven van een ronde meteen SQLITE_BUSY in
 * plaats van even te wachten.
 */
export function db(): DatabaseSync {
	if (connection) return connection;

	let opened: DatabaseSync;
	try {
		opened = new DatabaseSync(databasePath());
	} catch (error) {
		throw new ConfigurationError(
			`${databasePath()} kon niet geopend worden: ${(error as Error).message}. ` +
				'Klopt KAARTENJAGER_DB, en heeft dit proces leesrechten?'
		);
	}

	opened.exec('PRAGMA busy_timeout = 5000');
	opened.exec('PRAGMA foreign_keys = ON');

	const version = Number(
		(opened.prepare('PRAGMA user_version').get() as { user_version: number }).user_version
	);
	if (version === 0) {
		throw new ConfigurationError(
			`${databasePath()} bestaat nog niet of is leeg. Draai eerst \`kaartenjager run\` — ` +
				'dat maakt de database aan en zet de oude bestanden over.'
		);
	}
	if (version !== SCHEMA_VERSION) {
		throw new ConfigurationError(
			`${databasePath()} heeft schema ${version}, deze app kent ${SCHEMA_VERSION}. ` +
				'Werk het programma of de app bij; half werken op een onbekend schema is erger ' +
				'dan niet werken.'
		);
	}

	connection = opened;
	return connection;
}

export function nowSeconds(): number {
	return Math.floor(Date.now() / 1000);
}

// ------------------------------------------------------------------ app_state

export function readState(name: string): string | null {
	const row = db().prepare('SELECT value FROM app_state WHERE name = ?').get(name) as
		| { value: string }
		| undefined;
	return row?.value ?? null;
}

export function writeState(name: string, value: string): void {
	db()
		.prepare(
			`INSERT INTO app_state (name, value) VALUES (?, ?)
			 ON CONFLICT(name) DO UPDATE SET value = excluded.value`
		)
		.run(name, value);
}

/**
 * De belangrijkste weergave van de hele app. Een wachter die om is ziet er precies zo uit als
 * een markt zonder koopjes, dus dit verschil moet zichtbaar zijn zonder dat iemand eraan denkt
 * om te kijken.
 */
export function heartbeat(): Heartbeat {
	const raw = readState('last_round_at');
	const lastRoundAt = raw === null ? null : Number(raw);
	const hour = new Date().getHours();
	const withinDayWindow = hour >= DAY_STARTS_AT && hour <= DAY_ENDS_AT;

	let problems: string[] = [];
	try {
		problems = JSON.parse(readState('last_round_problems') ?? '[]');
	} catch {
		problems = [];
	}

	const stale =
		withinDayWindow &&
		(lastRoundAt === null || nowSeconds() - lastRoundAt > HEARTBEAT_STALE_SECONDS);

	return { lastRoundAt, problems, stale, withinDayWindow };
}

// -------------------------------------------------------------------- bezoek

export function lastVisit(): number {
	return Number(readState('last_visit') ?? 0);
}

/** De vorige stand blijft bewaard, zodat één klik terug te draaien is. */
export function markEverythingSeen(): void {
	writeState('previous_visit', String(lastVisit()));
	writeState('last_visit', String(nowSeconds()));
}

export function undoEverythingSeen(): void {
	const previous = readState('previous_visit');
	if (previous !== null) writeState('last_visit', previous);
}

// ------------------------------------------------------------------ vondsten

const FINDING_COLUMNS = `
	f.key, f.matched_as, f.kind, f.confidence, f.percent_under_market, f.euros_under_market,
	f.reasons, f.warnings, f.queue_note, f.became_a_find_at, f.judged_at, f.still_a_find,
	l.title, l.url, l.source, l.delivery, l.location, l.seller, l.condition, l.photo_count,
	l.description, l.gone_since,
	d.state, d.price_when_archived,
	(SELECT price_cents FROM price_point p WHERE p.key = f.key ORDER BY p.seen_at DESC LIMIT 1)
		AS price_cents,
	(SELECT asking_cents FROM price_point p WHERE p.key = f.key ORDER BY p.seen_at DESC LIMIT 1)
		AS asking_cents
`;

type Row = Record<string, string | number | null>;

function parseList(stored: unknown): string[] {
	if (typeof stored !== 'string') return [];
	try {
		const value = JSON.parse(stored);
		return Array.isArray(value) ? value : [];
	} catch {
		return [];
	}
}

function toFinding(row: Row, visitedAt: number): Finding {
	const key = String(row.key);
	const becameAFindAt = Number(row.became_a_find_at);
	const freshUntil = Math.max(visitedAt, nowSeconds() - STAYS_NEW_SECONDS);

	return {
		key,
		title: String(row.title),
		url: String(row.url),
		source: String(row.source),
		delivery: String(row.delivery),
		location: String(row.location ?? ''),
		seller: String(row.seller ?? ''),
		condition: String(row.condition ?? ''),
		photoCount: Number(row.photo_count ?? 0),
		description: String(row.description ?? ''),
		matchedAs: String(row.matched_as),
		kind: String(row.kind) as Kind,
		needsReview: row.confidence === 'review',
		percentUnderMarket: row.percent_under_market === null ? null : Number(row.percent_under_market),
		eurosUnderMarket: row.euros_under_market === null ? null : Number(row.euros_under_market),
		reasons: parseList(row.reasons),
		warnings: parseList(row.warnings),
		queueNote: row.queue_note === null ? null : String(row.queue_note),
		becameAFindAt,
		judgedAt: Number(row.judged_at),
		stillAFind: Number(row.still_a_find) === 1,
		goneSince: row.gone_since === null ? null : Number(row.gone_since),
		state: String(row.state ?? 'inbox') as State,
		priceEuros: Number(row.price_cents ?? 0) / 100,
		askingEuros: Number(row.asking_cents ?? 0) / 100,
		priceWhenArchived:
			row.price_when_archived === null ? null : Number(row.price_when_archived) / 100,
		isNew: becameAFindAt > freshUntil,
		priceMove: priceMove(key),
		review: latestReview(key)
	};
}

/** De laatste twee prijspunten. Daar draait "gezakt" op, en de terugkeer uit het archief. */
export function priceMove(key: string): PriceMove | null {
	const rows = db()
		.prepare(
			`SELECT seen_at, price_cents FROM price_point WHERE key = ?
			 ORDER BY seen_at DESC LIMIT 2`
		)
		.all(key) as { seen_at: number; price_cents: number }[];

	if (rows.length < 2) return null;
	const [latest, previous] = rows;
	if (latest.price_cents === previous.price_cents) return null;

	return {
		fromEuros: previous.price_cents / 100,
		toEuros: latest.price_cents / 100,
		days: Math.max(1, Math.round((latest.seen_at - previous.seen_at) / 86400))
	};
}

export function latestReview(key: string): Review | null {
	const row = db()
		.prepare(
			`SELECT id, requested_at, answered_at, verdict, recommendation, failed_reason
			 FROM review_request WHERE key = ? ORDER BY id DESC LIMIT 1`
		)
		.get(key) as Row | undefined;
	if (!row) return null;

	return {
		id: Number(row.id),
		requestedAt: Number(row.requested_at),
		answeredAt: row.answered_at === null ? null : Number(row.answered_at),
		verdict: row.verdict === null ? null : String(row.verdict),
		recommendation: row.recommendation === null ? null : String(row.recommendation),
		failedReason: row.failed_reason === null ? null : String(row.failed_reason)
	};
}

function query(where: string, order: string, limit: number, offset: number): Finding[] {
	const visitedAt = lastVisit();
	const rows = db()
		.prepare(
			`SELECT ${FINDING_COLUMNS}
			 FROM finding f
			 JOIN listing l ON l.key = f.key
			 LEFT JOIN decision d ON d.key = f.key
			 WHERE ${where}
			 ORDER BY ${order}
			 LIMIT ? OFFSET ?`
		)
		.all(limit, offset) as Row[];
	return rows.map((row) => toFinding(row, visitedAt));
}

/**
 * De inbox: wat nog leeft en wat je nog niet hebt weggelegd, plus wat uit het archief
 * terugkomt omdat de prijs meer dan tien procent zakte.
 *
 * Die terugkeer is met opzet een leesregel en geen statuswijziging: `decision.state` blijft
 * `archived` tot jij er iets mee doet, zodat de app en de scanner nooit allebei aan dezelfde
 * beslissing schrijven.
 */
export function inbox(limit: number, offset: number): Finding[] {
	return query(
		`f.still_a_find = 1
		 AND l.gone_since IS NULL
		 AND (
		   COALESCE(d.state, 'inbox') = 'inbox'
		   OR (d.state = 'archived'
		       AND d.price_when_archived IS NOT NULL
		       AND (SELECT price_cents FROM price_point p WHERE p.key = f.key
		            ORDER BY p.seen_at DESC LIMIT 1) < d.price_when_archived * ${RETURN_FROM_ARCHIVE_AT})
		 )`,
		'f.became_a_find_at DESC',
		limit,
		offset
	);
}

export function watching(limit: number, offset: number): Finding[] {
	return query(`d.state = 'watching'`, 'f.judged_at DESC', limit, offset);
}

/** Weggelegd, verdwenen, of niet langer interessant. De afvoer van de inbox. */
export function archive(limit: number, offset: number): Finding[] {
	return query(
		`(d.state = 'archived' OR l.gone_since IS NOT NULL OR f.still_a_find = 0)`,
		'f.judged_at DESC',
		limit,
		offset
	);
}

export function countInbox(): number {
	const row = db()
		.prepare(
			`SELECT COUNT(*) AS n FROM finding f
			 JOIN listing l ON l.key = f.key
			 LEFT JOIN decision d ON d.key = f.key
			 WHERE f.still_a_find = 1 AND l.gone_since IS NULL
			   AND COALESCE(d.state, 'inbox') = 'inbox'
			   AND f.became_a_find_at > ?`
		)
		.get(Math.max(lastVisit(), nowSeconds() - STAYS_NEW_SECONDS)) as { n: number };
	return Number(row.n);
}

export function setState(key: string, state: State): void {
	const price = db()
		.prepare(
			`SELECT price_cents FROM price_point WHERE key = ? ORDER BY seen_at DESC LIMIT 1`
		)
		.get(key) as { price_cents: number } | undefined;

	// De archiefprijs onthouden is het hele punt: zonder die waarde kan een latere daling
	// niet gezien worden, en dan komt de verkoper die na twee weken toegeeft nooit terug.
	db()
		.prepare(
			`INSERT INTO decision (key, state, changed_at, price_when_archived)
			 VALUES (?, ?, ?, ?)
			 ON CONFLICT(key) DO UPDATE SET
			   state = excluded.state,
			   changed_at = excluded.changed_at,
			   price_when_archived = CASE WHEN excluded.state = 'archived'
			                              THEN excluded.price_when_archived
			                              ELSE NULL END`
		)
		.run(key, state, nowSeconds(), state === 'archived' ? (price?.price_cents ?? null) : null);
}

// ------------------------------------------------------------------ wachtrij

/**
 * De knop. De unieke index `review_one_open` houdt tweemaal drukken tegen, dus die fout is
 * hier een bevestiging en geen storing.
 */
export function requestReview(key: string): { id: number; alreadyOpen: boolean } {
	const existing = db()
		.prepare(`SELECT id FROM review_request WHERE key = ? AND answered_at IS NULL`)
		.get(key) as { id: number } | undefined;
	if (existing) return { id: Number(existing.id), alreadyOpen: true };

	db()
		.prepare(`INSERT INTO review_request (key, requested_at) VALUES (?, ?)`)
		.run(key, nowSeconds());
	const created = db()
		.prepare(`SELECT id FROM review_request WHERE key = ? AND answered_at IS NULL`)
		.get(key) as { id: number };
	return { id: Number(created.id), alreadyOpen: false };
}

export function openReviewCount(): number {
	const row = db()
		.prepare('SELECT COUNT(*) AS n FROM review_request WHERE answered_at IS NULL')
		.get() as { n: number };
	return Number(row.n);
}

// ---------------------------------------------------------------- zoektermen

export function searchTerms(): SearchTerm[] {
	const rows = db()
		.prepare(
			`SELECT term, kind, enabled, added_at, added_by FROM search_term
			 ORDER BY CASE kind WHEN 'card' THEN 0 ELSE 1 END, term`
		)
		.all() as Row[];
	return rows.map((row) => ({
		term: String(row.term),
		kind: String(row.kind) as 'card' | 'part',
		enabled: Number(row.enabled) === 1,
		addedAt: Number(row.added_at),
		addedBy: String(row.added_by)
	}));
}

/**
 * Hoeveel termen er hoogstens aan mogen staan. Het programma schrijft dit weg, want het hangt
 * af van het aantal bronnen en dat staat in TOML — dat de app niet leest.
 */
export function maxEnabledTerms(): number {
	return Number(readState('max_search_terms') ?? 30);
}

export function enabledTermCount(): number {
	const row = db()
		.prepare('SELECT COUNT(*) AS n FROM search_term WHERE enabled = 1')
		.get() as { n: number };
	return Number(row.n);
}

/**
 * De grens valt hier, in het formulier, en niet pas in de ronde. Alleen daar weigeren zou
 * betekenen dat één extra term de wachter elk uur stilletjes laat weigeren.
 */
export function addTerm(term: string, kind: 'card' | 'part'): string | null {
	const cleaned = term.trim().toLowerCase();
	if (!cleaned) return 'Een lege zoekterm gaat niet.';
	if (cleaned.length > 60) return 'Die zoekterm is onwaarschijnlijk lang.';

	const exists = db().prepare('SELECT 1 FROM search_term WHERE term = ?').get(cleaned);
	if (exists) return `"${cleaned}" staat er al.`;

	const overTheLimit = tooManyTerms(1);
	if (overTheLimit) return overTheLimit;

	db()
		.prepare(
			`INSERT INTO search_term (term, kind, enabled, added_at, added_by)
			 VALUES (?, ?, 1, ?, 'app')`
		)
		.run(cleaned, kind, nowSeconds());
	return null;
}

export function toggleTerm(term: string, enabled: boolean): string | null {
	if (enabled) {
		const overTheLimit = tooManyTerms(1);
		if (overTheLimit) return overTheLimit;
	}
	db()
		.prepare('UPDATE search_term SET enabled = ? WHERE term = ?')
		.run(enabled ? 1 : 0, term);
	return null;
}

export function removeTerm(term: string): void {
	db().prepare('DELETE FROM search_term WHERE term = ?').run(term);
}

function tooManyTerms(extra: number): string | null {
	const maximum = maxEnabledTerms();
	if (enabledTermCount() + extra <= maximum) return null;
	return (
		`Dan staan er meer dan ${maximum} zoektermen aan, en dat is te veel voor één ronde. ` +
		'Zet er eerst een uit.'
	);
}
