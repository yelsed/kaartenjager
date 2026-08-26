<script lang="ts">
	import { enhance } from '$app/forms';
	import { ago } from '$lib/format';

	let { data, form } = $props();

	const ruimte = $derived(data.grens - data.aan);
</script>

{#if form?.message}
	<p class="melding" class:fout={!form.success}>{form.message}</p>
{/if}

<p class="uitleg">
	Dit is de enige configuratie die de app schrijft. Drempels, filters en kaartregels blijven in
	TOML op de server, waar een fout niet met één klik gemaakt is.
</p>

<p class="ruimte" class:vol={ruimte <= 0}>
	{data.aan} van de {data.grens} zoektermen staan aan.
	{#if ruimte <= 0}
		Er kan er geen meer bij; zet er eerst een uit.
	{:else if ruimte <= 3}
		Er passen er nog {ruimte}.
	{/if}
</p>

<form class="toevoegen" method="POST" action="?/toevoegen" use:enhance>
	<input name="term" placeholder="nieuwe zoekterm, ook de verkeerd gespelde" required />
	<select name="kind">
		<option value="card">kaart</option>
		<option value="part">onderdeel</option>
	</select>
	<button disabled={ruimte <= 0}>toevoegen</button>
</form>

<table>
	<thead>
		<tr>
			<th>Zoekterm</th>
			<th>Soort</th>
			<th>Toegevoegd</th>
			<th></th>
		</tr>
	</thead>
	<tbody>
		{#each data.termen as term (term.term)}
			<tr class:uit={!term.enabled}>
				<td>{term.term}</td>
				<td class="gedempt">{term.kind === 'card' ? 'kaart' : 'onderdeel'}</td>
				<td class="gedempt">{ago(term.addedAt)} · {term.addedBy}</td>
				<td>
					<!-- De knoppen zitten in een eigen laag: een tabelcel die zelf flex is,
					     tekent zijn randen niet meer door. -->
					<div class="acties">
						<form method="POST" action="?/aanzetten" use:enhance>
							<input type="hidden" name="term" value={term.term} />
							<input type="hidden" name="enabled" value={term.enabled ? 'nee' : 'ja'} />
							<button disabled={!term.enabled && ruimte <= 0}>
								{term.enabled ? 'uitzetten' : 'aanzetten'}
							</button>
						</form>
						<form method="POST" action="?/verwijderen" use:enhance>
							<input type="hidden" name="term" value={term.term} />
							<button class="weg">verwijderen</button>
						</form>
					</div>
				</td>
			</tr>
		{:else}
			<tr><td colspan="4" class="gedempt">Nog geen zoektermen.</td></tr>
		{/each}
	</tbody>
</table>

<p class="voetnoot">
	Een zoekterm uitzetten raakt bestaande vondsten niet: die worden via hun eigen
	advertentiepagina gevolgd, niet via de zoekresultaten.
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
	.voetnoot {
		color: var(--gedempt);
		font-size: 0.9rem;
	}

	.voetnoot {
		margin-top: 1.5rem;
	}

	.ruimte {
		font-size: 0.9rem;
		color: var(--gedempt);
	}

	.ruimte.vol {
		color: var(--let-op);
	}

	.toevoegen {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
		margin: 1rem 0 1.5rem;
	}

	.toevoegen input {
		flex: 1 1 16rem;
		font: inherit;
		padding: 0.35rem 0.6rem;
		border-radius: 6px;
		border: 1px solid var(--rand);
		background: var(--achtergrond);
		color: var(--tekst);
	}

	.toevoegen select {
		font: inherit;
		padding: 0.35rem 0.5rem;
		border-radius: 6px;
		border: 1px solid var(--rand);
		background: var(--vlak);
		color: var(--tekst);
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.9rem;
	}

	th {
		text-align: left;
		font-size: 0.75rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--gedempt);
		font-weight: 600;
		padding: 0 0.5rem 0.4rem 0;
	}

	td {
		padding: 0.45rem 0.5rem 0.45rem 0;
		border-top: 1px solid var(--rand);
		vertical-align: middle;
	}

	tr.uit td:first-child {
		color: var(--gedempt);
		text-decoration: line-through;
	}

	.gedempt {
		color: var(--gedempt);
	}

	.acties {
		display: flex;
		gap: 0.35rem;
		justify-content: flex-end;
	}

	.acties form {
		margin: 0;
	}

	button.weg:hover {
		border-color: var(--alarm);
		color: var(--alarm);
	}
</style>
