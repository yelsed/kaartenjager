<script lang="ts">
	import Vondst from '$lib/Vondst.svelte';
	import type { Finding } from '$lib/server/db';

	let {
		vondsten,
		erIsMeer,
		volgende,
		toon,
		leeg
	}: {
		vondsten: Finding[];
		erIsMeer: boolean;
		volgende: number;
		toon: 'inbox' | 'volglijst' | 'archief';
		leeg: string;
	} = $props();
</script>

{#if vondsten.length === 0}
	<p class="leeg">{leeg}</p>
{:else}
	{#each vondsten as vondst (vondst.key)}
		<Vondst {vondst} {toon} />
	{/each}
	{#if erIsMeer}
		<!-- Vijftig per keer. Duizenden regels in één pagina is traag zonder dat iemand er
		     iets aan heeft. -->
		<a class="meer" href="?toon={volgende}">meer laden</a>
	{/if}
{/if}

<style>
	.leeg {
		color: var(--gedempt);
		padding: 2rem 0;
	}

	.meer {
		display: inline-block;
		margin-top: 0.5rem;
		font-size: 0.9rem;
	}
</style>
