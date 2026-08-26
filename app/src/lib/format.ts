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
