<script lang="ts">
	import Lijst from '$lib/Lijst.svelte';

	let { data, form } = $props();
</script>

{#if form?.message}
	<p class="melding">{form.message}</p>
{/if}

<Lijst
	vondsten={data.vondsten}
	erIsMeer={data.erIsMeer}
	volgende={data.volgende}
	toon="volglijst"
	leeg="Nog niets op de volglijst. Bedoeld voor de twee of drie kaarten waar je serieus over nadenkt."
/>

<style>
	.melding {
		background: #1b2a24;
		border: 1px solid var(--accent);
		border-radius: 8px;
		padding: 0.6rem 0.9rem;
		margin: 0 0 1rem;
		font-size: 0.9rem;
	}
</style>
