const euros = new Intl.NumberFormat('nl-NL', {
	style: 'currency',
	currency: 'EUR',
	minimumFractionDigits: 2
});

const wholeEuros = new Intl.NumberFormat('nl-NL', {
	style: 'currency',
	currency: 'EUR',
	maximumFractionDigits: 0
});

export function money(amount: number): string {
	return euros.format(amount);
}

export function roundMoney(amount: number): string {
	return wholeEuros.format(amount);
}

export function percent(value: number): string {
	return `${Math.round(value)}%`;
}

/** "vandaag", "gisteren", "3 dagen geleden" — preciezer heeft hier geen nut. */
export function ago(seconds: number): string {
	const days = Math.floor((Date.now() / 1000 - seconds) / 86400);
	if (days <= 0) return 'vandaag';
	if (days === 1) return 'gisteren';
	if (days < 31) return `${days} dagen geleden`;
	const months = Math.round(days / 30);
	return months === 1 ? 'een maand geleden' : `${months} maanden geleden`;
}

export function clockTime(seconds: number): string {
	return new Date(seconds * 1000).toLocaleString('nl-NL', {
		weekday: 'short',
		hour: '2-digit',
		minute: '2-digit'
	});
}

export function sourceName(source: string): string {
	if (source === 'vinted') return 'Vinted';
	if (source === 'marktplaats') return 'Marktplaats';
	return source;
}

export function deliveryName(delivery: string): string {
	if (delivery === 'pickup') return 'alleen ophalen';
	if (delivery === 'shipping') return 'verzenden';
	return '';
}

/** "12 minuten", "3 uur 20", "2 dagen". Voor hoe lang iets online stond. */
export function duration(seconds: number): string {
	if (seconds < 60) return 'minder dan een minuut';

	const minutes = Math.round(seconds / 60);
	if (minutes < 60) return `${minutes} ${minutes === 1 ? 'minuut' : 'minuten'}`;

	const hours = Math.floor(minutes / 60);
	const rest = minutes % 60;
	if (hours < 24) {
		return rest === 0 ? `${hours} uur` : `${hours} uur ${rest} min`;
	}

	const days = Math.round(hours / 24);
	return `${days} ${days === 1 ? 'dag' : 'dagen'}`;
}

export function stamp(seconds: number): string {
	return new Date(seconds * 1000).toLocaleString('nl-NL', {
		day: 'numeric',
		month: 'short',
		hour: '2-digit',
		minute: '2-digit'
	});
}

/** "48 seconden", "2 minuten en 30 seconden". Voor hoe lang een ronde duurde. */
export function roundDuration(seconds: number): string {
	const whole = Math.round(seconds);
	const minutes = Math.floor(whole / 60);
	const rest = whole % 60;
	const secondsText = `${rest} ${rest === 1 ? 'seconde' : 'seconden'}`;
	if (minutes === 0) return secondsText;
	const minutesText = `${minutes} ${minutes === 1 ? 'minuut' : 'minuten'}`;
	return rest === 0 ? minutesText : `${minutesText} en ${secondsText}`;
}

/**
 * De kleuren waaruit je een product een kleur geeft. Allemaal licht genoeg om als tekst op de
 * donkere achtergrond leesbaar te blijven; een vrije kleurkiezer zou ook zwart toelaten.
 */
export const PRODUCT_COLORS = [
	{ name: 'blauw', value: '#86b0f5' },
	{ name: 'paars', value: '#c6a2f2' },
	{ name: 'roze', value: '#f093be' },
	{ name: 'turquoise', value: '#6fd0dc' },
	{ name: 'geel', value: '#e3d36e' },
	{ name: 'oranje', value: '#f2a47c' },
	{ name: 'limoen', value: '#a9d672' },
	{ name: 'zilver', value: '#c3cad5' }
] as const;

/** De gekozen kleur, anders die van zijn plaats in de configuratie; zonder product grijs. */
export function productColor(chosen: string | null, position: number | null): string {
	if (chosen && PRODUCT_COLORS.some((color) => color.value === chosen)) return chosen;
	if (position === null) return 'var(--gedempt)';
	return PRODUCT_COLORS[position % PRODUCT_COLORS.length].value;
}
