<script lang="ts">
	import { enhance } from '$app/forms';

	let { data, form } = $props();

	let inhoud = $state(data.inhoud);

	// Alleen bijstellen wanneer de server iets nieuws zegt — dus na een bewaarpoging, niet
	// tijdens het typen. Bij een afkeuring stuurt de server terug wat je getypt had, want
	// dat is precies wat nog gerepareerd moet worden; bij een geslaagde poging staat er de
	// versie zoals hij nu op schijf staat.
	$effect(() => {
		inhoud = form?.inhoud ?? data.inhoud;
	});

	const controle = $derived(form?.controle ?? data.controle);
	const gewijzigd = $derived(inhoud !== data.inhoud);
</script>

{#if form?.message}
	<p class="melding" class:fout={!form.success}>{form.message}</p>
{/if}

{#if data.probleem}
	<p class="melding fout">{data.probleem}</p>
{:else}
	<p class="uitleg">
		Dit is het bestand dat de wachter elke ronde leest: de prijstabel, de drempels, de
		filters en je machine. Zoektermen staan er ook in, maar die worden alleen bij de
		allereerste start overgenomen — daarna is het
		<a href="/zoektermen">tabblad Zoektermen</a> de baas over die lijst.
	</p>

	<p class="pad"><code>{data.pad}</code></p>

	<form method="POST" action="?/bewaren" use:enhance>
		<textarea name="inhoud" bind:value={inhoud} spellcheck="false" rows="28"></textarea>

		<div class="regel">
			<button type="submit" disabled={!gewijzigd}>Bewaren</button>
			<button type="button" class="terug" disabled={!gewijzigd} onclick={() => (inhoud = data.inhoud)}>
				Wijzigingen weggooien
			</button>
			<span class="gedempt">
				{#if gewijzigd}
					Nog niet bewaard. Er wordt niets weggeschreven wat de controle afkeurt.
				{:else}
					Gelijk aan wat er op de server staat.
				{/if}
			</span>
		</div>
	</form>
{/if}

{#if controle}
	<h2>Wat de controle zegt</h2>
	<pre class:fout={form && !form.success}>{controle}</pre>
{/if}

<p class="voetnoot">
	Bewaren draait <code>kaartenjager check</code> op de nieuwe versie voordat er iets
	vervangen wordt, dus een typefout kan de wachter niet stilleggen. De versie van vóór het
	bewaren blijft naast het bestand staan als <code>.vorige</code>.
</p>

<style>
	.melding {
		background: #1b2a24;
		border: 1px solid var(--accent);
		border-radius: 8px;
		padding: 0.6rem 0.9rem;
		margin: 0 0 1rem;
		font-size: 0.9rem;
	}

	.melding.fout {
		background: #33191c;
		border-color: var(--alarm);
	}

	.uitleg,
	.voetnoot,
	.gedempt {
		color: var(--gedempt);
		font-size: 0.9rem;
	}

	.voetnoot {
		margin-top: 1.5rem;
	}

	.pad {
		font-size: 0.8rem;
		color: var(--gedempt);
		margin: 0 0 0.5rem;
	}

	textarea {
		width: 100%;
		box-sizing: border-box;
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
		font-size: 0.82rem;
		line-height: 1.5;
		tab-size: 2;
		padding: 0.7rem 0.9rem;
		border-radius: 8px;
		border: 1px solid var(--rand);
		background: var(--achtergrond);
		color: var(--tekst);
		resize: vertical;
	}

	.regel {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.6rem;
		margin-top: 0.7rem;
	}

	button:disabled {
		opacity: 0.45;
		cursor: default;
	}

	button.terug:hover:not(:disabled) {
		border-color: var(--alarm);
		color: var(--alarm);
	}

	h2 {
		font-size: 0.75rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--gedempt);
		font-weight: 600;
		margin: 1.5rem 0 0.4rem;
	}

	pre {
		font-size: 0.8rem;
		line-height: 1.5;
		white-space: pre-wrap;
		word-break: break-word;
		margin: 0;
		padding: 0.7rem 0.9rem;
		border-radius: 8px;
		border: 1px solid var(--rand);
		background: var(--vlak);
		color: var(--gedempt);
	}

	pre.fout {
		border-color: var(--alarm);
		color: var(--tekst);
	}
</style>
