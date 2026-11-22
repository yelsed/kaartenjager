<script lang="ts">
	import ProductSwitch from '$lib/ProductSwitch.svelte';

	let { data, form } = $props();
</script>

{#if form?.success && form.message}
	<p class="melding">{form.message}</p>
{/if}

<p class="uitleg">
	Een product dat uit staat wordt niet afgezocht, meldt niets en staat niet in de inbox. Zijn
	vondsten blijven bewaard. Een nieuw product voeg je toe in de configuratie, onder
	<a href="/instellingen">Instellingen</a>.
</p>

<ul>
	{#each data.producten as product (product.id)}
		<ProductSwitch {product} />
	{:else}
		<li class="gedempt">Nog geen producten. Draai eerst een ronde.</li>
	{/each}
</ul>

<style>
	.melding {
		background: #1b2a24;
		border: 1px solid var(--accent);
		border-radius: 8px;
		padding: 0.6rem 0.9rem;
		margin: 0 0 1rem;
		font-size: 0.9rem;
	}

	.uitleg,
	.gedempt {
		color: var(--gedempt);
		font-size: 0.9rem;
	}

	ul {
		list-style: none;
		padding: 0;
		margin: 1rem 0 0;
	}
</style>
