<script lang="ts">
	import { enhance } from '$app/forms';
	import Lijst from '$lib/Lijst.svelte';

	let { data, form } = $props();

	const nieuwe = $derived(data.vondsten.filter((vondst) => vondst.isNew));
	const eerder = $derived(data.vondsten.filter((vondst) => !vondst.isNew));
</script>

{#if form?.message}
	<p class="melding">{form.message}</p>
{/if}

{#if nieuwe.length > 0}
	<div class="blokkop">
		<h2>Nieuw</h2>
		<!--
			Bij het verlaten van de pagina bijzetten klinkt logisch maar werkt niet: beforeunload
			gaat niet af bij een tabblad dat op een telefoon gesloten wordt. Dus een knop, plus
			een grens van achtenveertig uur die vanzelf loopt.
		-->
		<form method="POST" action="?/allesGezien" use:enhance>
			<button>alles gezien</button>
		</form>
	</div>
	<Lijst
		vondsten={nieuwe}
		erIsMeer={false}
		volgende={data.volgende}
		toon="inbox"
		leeg="Niets nieuws."
	/>
{:else}
	<div class="blokkop">
		<h2>Niets nieuws sinds je laatste bezoek</h2>
		<form method="POST" action="?/toonWeerAlsNieuw" use:enhance>
			<button>toch weer tonen</button>
		</form>
	</div>
{/if}

{#if eerder.length > 0}
	<h2 class="eerder">Eerder</h2>
	<Lijst
		vondsten={eerder}
		erIsMeer={data.erIsMeer}
		volgende={data.volgende}
		toon="inbox"
		leeg=""
	/>
{/if}

<style>
	.melding {
		background: #1b2a24;
		border: 1px solid var(--accent);
		border-radius: 8px;
		padding: 0.6rem 0.9rem;
		margin: 0 0 1rem;
		font-size: 0.9rem;
	}

	.blokkop {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 1rem;
		margin: 0 0 0.8rem;
	}

	h2 {
		font-size: 0.8rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--gedempt);
		margin: 0;
	}

	h2.eerder {
		margin: 2rem 0 0.8rem;
	}
</style>
