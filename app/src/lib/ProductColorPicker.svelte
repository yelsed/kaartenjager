<script lang="ts">
	import { enhance } from '$app/forms';
	import { PRODUCT_COLORS } from '$lib/format';
	import type { Product } from '$lib/server/db';

	let { product }: { product: Product } = $props();

	let busy = $state(false);
	let error = $state<string | null>(null);
</script>

<details>
	<summary>kleur kiezen</summary>
	<form
		method="POST"
		action="/producten?/kleur"
		use:enhance={() => {
			busy = true;
			error = null;
			return async ({ result, update }) => {
				busy = false;
				if (result.type === 'failure') error = String(result.data?.message ?? 'Mislukt.');
				await update({ reset: false });
			};
		}}
	>
		<input type="hidden" name="id" value={product.id} />
		<div class="palet" role="group" aria-label="Kleur van {product.name}">
			{#each PRODUCT_COLORS as color (color.value)}
				<button
					name="color"
					value={color.value}
					aria-label={color.name}
					aria-pressed={product.color === color.value}
					title={color.name}
					disabled={busy}
					style:--staal={color.value}
				></button>
			{/each}
		</div>
	</form>
	{#if error}<p class="fout">{error}</p>{/if}
</details>

<style>
	details {
		margin-top: 0.35rem;
	}

	summary {
		width: fit-content;
		font-size: 0.85rem;
		color: var(--gedempt);
		cursor: pointer;
	}

	form {
		margin: 0.5rem 0 0;
	}

	.palet {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}

	button {
		width: 2.25rem;
		height: 2.25rem;
		border-radius: 50%;
		border: 2px solid transparent;
		background: var(--staal);
		background-clip: content-box;
		padding: 3px;
	}

	button[aria-pressed='true'] {
		border-color: var(--tekst);
	}

	button:hover {
		border-color: var(--gedempt);
	}

	.fout {
		margin: 0.35rem 0 0;
		font-size: 0.85rem;
		color: var(--alarm);
	}
</style>
