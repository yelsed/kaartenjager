<script lang="ts">
	import { page } from '$app/state';
	import { clockTime } from '$lib/format';

	let { children, data } = $props();

	const tabbladen = [
		{ pad: '/', naam: 'Inbox' },
		{ pad: '/volglijst', naam: 'Volglijst' },
		{ pad: '/archief', naam: 'Archief' },
		{ pad: '/zoektermen', naam: 'Zoektermen' }
	];
</script>

<svelte:head>
	<title>Kaartenjager</title>
</svelte:head>

<div class="schil">
	{#if data.heartbeat.stale}
		<!--
			Een wachter die om is ziet er precies zo uit als een markt zonder koopjes. Dit is
			het enige verschil dat je te zien krijgt, dus het staat bovenaan en het is rood.
		-->
		<div class="alarm" role="alert">
			<strong>De wachter draait niet.</strong>
			{#if data.heartbeat.lastRoundAt}
				De laatste ronde was {clockTime(data.heartbeat.lastRoundAt)}, en dat is te lang
				geleden voor dit tijdstip.
			{:else}
				Er heeft nog nooit een ronde gedraaid.
			{/if}
			<span class="hint">Kijk op de server naar de cronjob <code>kaartenjager-scan</code>.</span>

			{#if data.heartbeat.problems.length > 0}
				<ul>
					{#each data.heartbeat.problems as probleem (probleem)}
						<li>{probleem}</li>
					{/each}
				</ul>
			{/if}
		</div>
	{:else if data.heartbeat.problems.length > 0}
		<div class="waarschuwing">
			<strong>De laatste ronde liep, maar niet vlekkeloos.</strong>
			<ul>
				{#each data.heartbeat.problems as probleem (probleem)}
					<li>{probleem}</li>
				{/each}
			</ul>
		</div>
	{/if}

	<header>
		<h1>Kaartenjager</h1>
		<nav>
			{#each tabbladen as tabblad (tabblad.pad)}
				<a
					href={tabblad.pad}
					class:actief={page.url.pathname === tabblad.pad}
					aria-current={page.url.pathname === tabblad.pad ? 'page' : undefined}
				>
					{tabblad.naam}
					{#if tabblad.pad === '/' && data.nieuw > 0}
						<span class="teller">{data.nieuw}</span>
					{/if}
				</a>
			{/each}
		</nav>
		{#if data.openVerzoeken > 0}
			<p class="wachtrij">
				{data.openVerzoeken === 1
					? 'Eén beoordeling wacht op Hermes.'
					: `${data.openVerzoeken} beoordelingen wachten op Hermes.`}
			</p>
		{/if}
	</header>

	<main>
		{@render children()}
	</main>

	<footer>
		{#if data.heartbeat.lastRoundAt && !data.heartbeat.stale}
			Laatste ronde {clockTime(data.heartbeat.lastRoundAt)}.
		{/if}
	</footer>
</div>

<style>
	:global(:root) {
		--achtergrond: #14161a;
		--vlak: #1c2027;
		--rand: #2b323c;
		--tekst: #e6e9ee;
		--gedempt: #98a1ae;
		--accent: #7fd1a8;
		--alarm: #e5646d;
		--let-op: #e0a458;
		color-scheme: dark;
	}

	:global(body) {
		margin: 0;
		background: var(--achtergrond);
		color: var(--tekst);
		font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
		line-height: 1.5;
	}

	:global(a) {
		color: var(--accent);
	}

	:global(button) {
		font: inherit;
		cursor: pointer;
		border: 1px solid var(--rand);
		background: var(--vlak);
		color: var(--tekst);
		border-radius: 6px;
		padding: 0.35rem 0.75rem;
	}

	:global(button:hover) {
		border-color: var(--accent);
	}

	.schil {
		max-width: 46rem;
		margin: 0 auto;
		padding: 1.5rem 1rem 4rem;
	}

	header {
		margin-bottom: 1.5rem;
	}

	h1 {
		font-size: 1.1rem;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--gedempt);
		margin: 0 0 0.75rem;
	}

	nav {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem;
		border-bottom: 1px solid var(--rand);
	}

	nav a {
		padding: 0.5rem 0.9rem;
		text-decoration: none;
		color: var(--gedempt);
		border-bottom: 2px solid transparent;
		margin-bottom: -1px;
	}

	nav a.actief {
		color: var(--tekst);
		border-bottom-color: var(--accent);
	}

	.teller {
		display: inline-block;
		margin-left: 0.35rem;
		padding: 0 0.4rem;
		border-radius: 999px;
		background: var(--accent);
		color: #10231b;
		font-size: 0.75rem;
		font-weight: 700;
	}

	.wachtrij {
		margin: 0.75rem 0 0;
		font-size: 0.9rem;
		color: var(--gedempt);
	}

	.alarm,
	.waarschuwing {
		border-radius: 8px;
		padding: 0.9rem 1rem;
		margin-bottom: 1.25rem;
	}

	.alarm {
		background: #33191c;
		border: 1px solid var(--alarm);
	}

	.waarschuwing {
		background: #2b2418;
		border: 1px solid var(--let-op);
	}

	.alarm ul,
	.waarschuwing ul {
		margin: 0.5rem 0 0;
		padding-left: 1.1rem;
		font-size: 0.9rem;
		color: var(--gedempt);
	}

	.hint {
		display: block;
		margin-top: 0.35rem;
		font-size: 0.9rem;
		color: var(--gedempt);
	}

	code {
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
		font-size: 0.9em;
	}

	footer {
		margin-top: 3rem;
		font-size: 0.85rem;
		color: var(--gedempt);
	}
</style>
