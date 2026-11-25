<script lang="ts">
	import type { InboxFilters } from '$lib/server/db';

	let { filters, product }: { filters: InboxFilters; product: string | null } = $props();

	const activeCount = $derived(Object.values(filters).filter((value) => value !== null).length);
</script>

<details class="inbox-filter" open={activeCount > 0}>
	<summary>
		Filteren
		{#if activeCount > 0}<span class="aantal">{activeCount} actief</span>{/if}
	</summary>
	<form method="GET">
		{#if product}<input type="hidden" name="product" value={product} />{/if}
		<label>
			Prijs vanaf
			<input type="number" name="prijsVanaf" min="0" step="1" inputmode="numeric" value={filters.minimumPrice ?? ''} />
		</label>
		<label>
			Prijs tot
			<input type="number" name="prijsTot" min="0" step="1" inputmode="numeric" value={filters.maximumPrice ?? ''} />
		</label>
		<label>
			CPU Mark per euro vanaf
			<input
				type="number"
				name="scoreVanaf"
				min="0"
				step="1"
				inputmode="numeric"
				value={filters.minimumScorePerEuro ?? ''}
			/>
		</label>
		<label>
			Geheugen
			<select name="geheugenVanaf" value={filters.minimumMemoryGb === null ? '' : String(filters.minimumMemoryGb)}>
				<option value="">alles</option>
				<option value="16">16 GB of meer</option>
				<option value="32">32 GB of meer</option>
				<option value="64">64 GB of meer</option>
			</select>
		</label>
		<label>
			Trap
			<select name="trap" value={filters.level ?? ''}>
				<option value="">alles</option>
				<option value="top">topvondst en ultra</option>
				<option value="ultra">alleen ultra</option>
			</select>
		</label>
		<div class="knoppen">
			<button>toepassen</button>
			{#if activeCount > 0}
				<a href={product ? `/?product=${encodeURIComponent(product)}` : '/'}>wissen</a>
			{/if}
		</div>
	</form>
</details>

<style>
	.inbox-filter {
		margin: 0 0 1.25rem;
		border: 1px solid var(--rand);
		border-radius: 8px;
		padding: 0.5rem 0.75rem;
		font-size: 0.9rem;
	}

	summary {
		cursor: pointer;
		color: var(--gedempt);
	}

	.aantal {
		margin-left: 0.4rem;
		color: var(--accent);
	}

	form {
		display: grid;
		grid-template-columns: 1fr;
		gap: 0.6rem;
		margin-top: 0.75rem;
	}

	@media (min-width: 40rem) {
		form {
			grid-template-columns: repeat(3, 1fr);
		}
	}

	label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		color: var(--gedempt);
		font-size: 0.8rem;
	}

	input,
	select {
		font: inherit;
		font-size: 0.9rem;
		padding: 0.35rem 0.5rem;
		border-radius: 6px;
		border: 1px solid var(--rand);
		background: var(--achtergrond);
		color: var(--tekst);
	}

	.knoppen {
		display: flex;
		align-items: end;
		gap: 0.75rem;
	}
</style>
