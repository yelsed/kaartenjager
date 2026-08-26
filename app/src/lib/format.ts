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
