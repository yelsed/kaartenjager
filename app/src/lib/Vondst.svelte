<script lang="ts">
	import { enhance } from '$app/forms';
	import type { Finding } from '$lib/server/db';
	import {
		money,
		roundMoney,
		percent,
		ago,
		duration,
		stamp,
		sourceName,
		deliveryName
	} from '$lib/format';

	let { vondst, toon = 'inbox' }: { vondst: Finding; toon?: 'inbox' | 'volglijst' | 'archief' } =
		$props();

	let uitgeklapt = $state(false);

	// Drie regels in de lijst: wat, hoeveel, waarom. Al het andere zit achter het uitklappen —
	// dat is het verschil met Discord, waar alles altijd openstond.
	const kenmerken = $derived(
		[
			vondst.title,
			vondst.condition,
			vondst.location,
			deliveryName(vondst.delivery),
			sourceName(vondst.source)
		].filter(Boolean)
	);

	// Hoe lang de advertentie te koop stond. Vanaf het plaatsen als de bron dat prijsgeeft,
	// anders vanaf het moment dat wij hem zagen — en dat is een ondergrens, geen exacte duur.
	const beginpunt = $derived(vondst.postedAt ?? vondst.becameAFindAt);
	const eindpunt = $derived(vondst.goneSince ?? vondst.lastAlive);
	const standtijd = $derived(Math.max(0, eindpunt - beginpunt));

	// De belangstelling van de eerste tot de laatste waarneming.
	// Vinted stuurt view_count wel mee, maar in zoekresultaten staat hij altijd op nul. Een
	// kolom met alleen nullen is ruis, dus die tonen we pas als er echt iets in staat.
	const kijkers = $derived(vondst.sightings.filter((s) => (s.viewCount ?? 0) > 0));
	const favorieten = $derived(vondst.sightings.filter((s) => s.favouriteCount !== null));

	const terugUitArchief = $derived(
		vondst.state === 'archived' &&
			vondst.priceWhenArchived !== null &&
			vondst.priceEuros < vondst.priceWhenArchived * 0.9
	);
</script>

<article class:nieuw={vondst.isNew}>
	<header>
		<div class="kop">
			<h3>{vondst.matchedAs}</h3>
			<span class="prijs">{money(vondst.priceEuros)}</span>
		</div>
		<div class="merken">
			{#if vondst.isNew}<span class="merk nieuw-merk">nieuw</span>{/if}
			{#if vondst.needsReview}<span class="merk uitzoeken">uitzoeken</span>{/if}
			{#if vondst.percentUnderMarket !== null && vondst.percentUnderMarket > 0}
				<span class="merk">{percent(vondst.percentUnderMarket)} onder de markt</span>
			{/if}
			{#if vondst.goneSince}
				<!-- Verkocht is iets anders dan weggehaald: het eerste zegt dat iemand anders
				     hem zag, het tweede dat de verkoper zich bedacht. -->
				<span class="merk weg">
					{vondst.goneReason === 'sold' ? 'verkocht' : 'weggehaald'}
					{ago(vondst.goneSince)}
				</span>
			{/if}
			{#if !vondst.stillAFind}<span class="merk weg">niet langer interessant</span>{/if}
		</div>
	</header>

	<p class="kenmerken">{kenmerken.join(' · ')}</p>

	{#if terugUitArchief && vondst.priceWhenArchived !== null}
		<p class="terug">
			Was gearchiveerd op {roundMoney(vondst.priceWhenArchived)}, staat nu op
			{roundMoney(vondst.priceEuros)}.
		</p>
	{:else if vondst.priceMove}
		<p class="beweging">
			{roundMoney(vondst.priceMove.fromEuros)} → {roundMoney(vondst.priceMove.toEuros)} in
			{vondst.priceMove.days === 1 ? 'een dag' : `${vondst.priceMove.days} dagen`}
		</p>
	{/if}

	{#if vondst.queueNote}
		<p class="notitie">{vondst.queueNote}</p>
	{/if}

	{#if vondst.review?.answeredAt && vondst.review.verdict}
		<div class="oordeel" data-aanbeveling={vondst.review.recommendation}>
			<strong>Hermes: {vondst.review.recommendation}</strong>
			<p>{vondst.review.verdict}</p>
		</div>
	{:else if vondst.review?.failedReason}
		<div class="oordeel mislukt">
			<strong>Beoordeling mislukt</strong>
			<p>{vondst.review.failedReason}</p>
		</div>
	{:else if vondst.review && !vondst.review.answeredAt}
		<p class="wacht">Wacht sinds {ago(vondst.review.requestedAt)} op Hermes.</p>
	{/if}

	{#if uitgeklapt}
		<div class="dossier">
			{#if vondst.reasons.length > 0}
				<h4>Waarom interessant</h4>
				<ul>
					{#each vondst.reasons as reden (reden)}<li>{reden}</li>{/each}
				</ul>
			{/if}
			{#if vondst.warnings.length > 0}
				<h4>Let op</h4>
				<ul class="let-op">
					{#each vondst.warnings as waarschuwing (waarschuwing)}<li>{waarschuwing}</li>{/each}
				</ul>
			{/if}
			{#if vondst.description}
				<h4>De verkoper schrijft</h4>
				<p class="beschrijving">{vondst.description}</p>
			{/if}
			<h4>Hoe het verliep</h4>
			<dl class="tijdlijn">
				{#if vondst.postedAt}
					<dt>Geplaatst</dt>
					<dd>{stamp(vondst.postedAt)}</dd>
				{/if}
				<dt>Gevonden</dt>
				<dd>
					{stamp(vondst.becameAFindAt)}
					{#if vondst.postedAt}
						<span class="na">{duration(vondst.becameAFindAt - vondst.postedAt)} na plaatsen</span>
					{/if}
				</dd>
				{#if vondst.goneSince}
					<dt>{vondst.goneReason === 'sold' ? 'Verkocht' : 'Weggehaald'}</dt>
					<dd>{stamp(vondst.goneSince)}</dd>
				{:else}
					<dt>Laatst gezien</dt>
					<dd>{stamp(vondst.lastAlive)}</dd>
				{/if}
				<dt>{vondst.goneSince ? 'Stond online' : 'Staat er nu'}</dt>
				<dd>
					{duration(standtijd)}
					{#if !vondst.postedAt}<span class="na">minstens — plaatsingstijd onbekend</span>{/if}
				</dd>
			</dl>

			{#if kijkers.length > 1 || favorieten.length > 1}
				<h4>Belangstelling</h4>
				<ul class="belangstelling">
					{#each vondst.sightings as waarneming (waarneming.seenAt)}
						<li>
							<span class="tijd">{stamp(waarneming.seenAt)}</span>
							<span>{money(waarneming.priceEuros)}</span>
							{#if (waarneming.viewCount ?? 0) > 0}
								<span>{waarneming.viewCount} keer bekeken</span>
							{/if}
							{#if waarneming.favouriteCount !== null}
								<span>{waarneming.favouriteCount}× bewaard</span>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}

			<p class="klein">
				{vondst.photoCount}
				{vondst.photoCount === 1 ? "foto" : "foto's"}
				{#if vondst.seller} · verkoper {vondst.seller}{/if}
			</p>
		</div>
	{/if}

	<div class="knoppen">
		{#if toon !== 'archief'}
			<form method="POST" action="?/archiveren" use:enhance>
				<input type="hidden" name="key" value={vondst.key} />
				<button>archiveren</button>
			</form>
		{/if}
		{#if toon !== 'volglijst'}
			<form method="POST" action="?/volgen" use:enhance>
				<input type="hidden" name="key" value={vondst.key} />
				<button>volgen</button>
			</form>
		{/if}
		{#if toon !== 'inbox'}
			<form method="POST" action="?/terug" use:enhance>
				<input type="hidden" name="key" value={vondst.key} />
				<button>terug naar inbox</button>
			</form>
		{/if}
		{#if !vondst.review || vondst.review.answeredAt}
			<form method="POST" action="?/hermes" use:enhance>
				<input type="hidden" name="key" value={vondst.key} />
				<button class:uitgelicht={vondst.needsReview}>Hermes laten kijken</button>
			</form>
		{/if}
		<button type="button" onclick={() => (uitgeklapt = !uitgeklapt)}>
			{uitgeklapt ? 'inklappen' : 'uitklappen'}
		</button>
		<a class="link" href={vondst.url} target="_blank" rel="noreferrer noopener">openen →</a>
	</div>
</article>

<style>
	article {
		border: 1px solid var(--rand);
		border-radius: 10px;
		padding: 1rem;
		margin-bottom: 0.9rem;
		background: var(--vlak);
	}

	article.nieuw {
		border-left: 3px solid var(--accent);
	}

	.kop {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 1rem;
	}

	h3 {
		margin: 0;
		font-size: 1.05rem;
	}

	.prijs {
		font-variant-numeric: tabular-nums;
		font-weight: 600;
		white-space: nowrap;
	}

	.merken {
		display: flex;
		flex-wrap: wrap;
		gap: 0.35rem;
		margin-top: 0.4rem;
	}

	.merk {
		font-size: 0.75rem;
		padding: 0.1rem 0.5rem;
		border-radius: 999px;
		border: 1px solid var(--rand);
		color: var(--gedempt);
	}

	.merk.nieuw-merk {
		border-color: var(--accent);
		color: var(--accent);
	}

	.merk.uitzoeken {
		border-color: var(--let-op);
		color: var(--let-op);
	}

	.merk.weg {
		border-color: var(--alarm);
		color: var(--alarm);
	}

	.kenmerken {
		margin: 0.6rem 0 0;
		color: var(--gedempt);
		font-size: 0.9rem;
	}

	.terug {
		margin: 0.5rem 0 0;
		color: var(--accent);
		font-size: 0.9rem;
	}

	.beweging,
	.wacht {
		margin: 0.5rem 0 0;
		color: var(--gedempt);
		font-size: 0.9rem;
		font-variant-numeric: tabular-nums;
	}

	.notitie {
		margin: 0.5rem 0 0;
		color: var(--let-op);
		font-size: 0.9rem;
	}

	.oordeel {
		margin-top: 0.75rem;
		padding: 0.6rem 0.8rem;
		border-radius: 8px;
		border-left: 3px solid var(--gedempt);
		background: #191d24;
		font-size: 0.9rem;
	}

	.oordeel[data-aanbeveling='kijken'] {
		border-left-color: var(--accent);
	}

	.oordeel[data-aanbeveling='overslaan'] {
		border-left-color: var(--gedempt);
	}

	.oordeel[data-aanbeveling='oplichterij'],
	.oordeel.mislukt {
		border-left-color: var(--alarm);
	}

	.oordeel p {
		margin: 0.35rem 0 0;
		white-space: pre-wrap;
	}

	.dossier {
		margin-top: 0.9rem;
		padding-top: 0.9rem;
		border-top: 1px solid var(--rand);
		font-size: 0.9rem;
	}

	.dossier h4 {
		margin: 0.8rem 0 0.3rem;
		font-size: 0.75rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--gedempt);
	}

	.dossier h4:first-child {
		margin-top: 0;
	}

	.dossier ul {
		margin: 0;
		padding-left: 1.1rem;
	}

	.dossier ul.let-op {
		color: var(--let-op);
	}

	.beschrijving {
		margin: 0;
		white-space: pre-wrap;
		color: var(--gedempt);
	}

	.klein {
		margin: 0.8rem 0 0;
		color: var(--gedempt);
		font-size: 0.8rem;
	}

	.tijdlijn {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0.15rem 0.9rem;
		margin: 0;
	}

	.tijdlijn dt {
		color: var(--gedempt);
	}

	.tijdlijn dd {
		margin: 0;
		font-variant-numeric: tabular-nums;
	}

	.na {
		color: var(--gedempt);
		margin-left: 0.4rem;
	}

	.belangstelling {
		list-style: none;
		margin: 0;
		padding: 0;
		font-variant-numeric: tabular-nums;
	}

	.belangstelling li {
		display: flex;
		flex-wrap: wrap;
		gap: 0.9rem;
		padding: 0.15rem 0;
		border-bottom: 1px solid var(--rand);
	}

	.belangstelling li:last-child {
		border-bottom: none;
	}

	.belangstelling .tijd {
		color: var(--gedempt);
		min-width: 8.5rem;
	}

	.knoppen {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.4rem;
		margin-top: 0.9rem;
	}

	.knoppen form {
		margin: 0;
	}

	button.uitgelicht {
		border-color: var(--let-op);
		color: var(--let-op);
	}

	.link {
		margin-left: auto;
		font-size: 0.9rem;
		text-decoration: none;
	}
</style>
