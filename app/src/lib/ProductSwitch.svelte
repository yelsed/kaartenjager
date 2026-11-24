<script lang="ts">
	import { enhance } from '$app/forms';
	import type { Product } from '$lib/server/db';
	import ProductLabel from '$lib/ProductLabel.svelte';
	import ProductColorPicker from '$lib/ProductColorPicker.svelte';

	let { product }: { product: Product } = $props();

	let busy = $state(false);
	let error = $state<string | null>(null);
</script>

<li class:uit={!product.enabled}>
	<div class="omschrijving">
		<strong><ProductLabel name={product.name} color={product.color} /></strong>
		<span class="gedempt">
			{product.enabledTermCount === 1 ? '1 zoekterm' : `${product.enabledTermCount} zoektermen`}
			· {product.inboxCount}
			{product.enabled ? 'in de inbox' : 'bewaard voor als hij weer aan gaat'}
		</span>
		{#if error}<span class="fout">{error}</span>{/if}
		<ProductColorPicker {product} />
	</div>
	<form
		method="POST"
		action="/producten?/aanzetten"
		use:enhance={() => {
			busy = true;
			error = null;
			return async ({ result, update }) => {
				busy = false;
				if (result.type === 'failure') error = String(result.data?.message ?? 'Mislukt.');
				await update();
			};
		}}
	>
		<input type="hidden" name="id" value={product.id} />
		<input type="hidden" name="enabled" value={product.enabled ? 'nee' : 'ja'} />
		<button role="switch" aria-checked={product.enabled} disabled={busy}>
			{product.enabled ? 'aan' : 'uit'}
		</button>
	</form>
</li>

<style>
	li {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.75rem 0;
		border-top: 1px solid var(--rand);
	}

	.omschrijving {
		display: flex;
		flex-direction: column;
	}

	li.uit strong {
		opacity: 0.55;
	}

	.gedempt {
		font-size: 0.85rem;
		color: var(--gedempt);
	}

	.fout {
		font-size: 0.85rem;
		color: var(--alarm);
	}

	form {
		margin: 0;
	}

	button {
		min-width: 4rem;
	}

	button[aria-checked='true'] {
		border-color: var(--accent);
		color: var(--accent);
	}
</style>
