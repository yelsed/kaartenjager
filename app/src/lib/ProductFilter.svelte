<script lang="ts">
	import type { Product } from '$lib/server/db';

	let {
		products,
		selected,
		totalCount
	}: { products: Product[]; selected: string | null; totalCount: number } = $props();

	const enabledProducts = $derived(products.filter((product) => product.enabled));
</script>

{#if enabledProducts.length > 1}
	<nav class="product-filter" aria-label="Product">
		<a href="/" class:actief={selected === null} aria-current={selected === null ? 'page' : undefined}>
			Alles <span class="aantal">{totalCount}</span>
		</a>
		{#each enabledProducts as product (product.id)}
			<a
				href="/?product={encodeURIComponent(product.id)}"
				class:actief={selected === product.id}
				aria-current={selected === product.id ? 'page' : undefined}
			>
				{product.name} <span class="aantal">{product.inboxCount}</span>
			</a>
		{/each}
	</nav>
{/if}

<style>
	.product-filter {
		display: flex;
		gap: 0.4rem;
		overflow-x: auto;
		padding-bottom: 0.25rem;
		margin: 0 0 1.25rem;
	}

	a {
		flex: none;
		padding: 0.3rem 0.75rem;
		border: 1px solid var(--rand);
		border-radius: 999px;
		font-size: 0.85rem;
		color: var(--gedempt);
		text-decoration: none;
		white-space: nowrap;
	}

	a.actief {
		border-color: var(--accent);
		color: var(--tekst);
	}

	.aantal {
		margin-left: 0.2rem;
		font-variant-numeric: tabular-nums;
		color: var(--gedempt);
	}
</style>
