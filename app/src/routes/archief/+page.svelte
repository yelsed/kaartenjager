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
	toon="archief"
	leeg="Het archief is leeg. Hier komt wat je weglegt, plus wat verdwijnt of te duur wordt."
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
