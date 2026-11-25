<script lang="ts">
	import type { ReferenceMachine } from '$lib/server/db';

	let { machine }: { machine: ReferenceMachine } = $props();
</script>

<div class="reference-machine">
	<span class="label">Jouw machine</span>
	<strong>{machine.name}</strong>
	<span class="score">CPU Mark {Math.round(machine.cpuMark).toLocaleString('nl-NL')}</span>
</div>

<style>
	.reference-machine {
		position: sticky;
		top: 0;
		z-index: 10;
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0.25rem 0.6rem;
		margin: 0 0 1rem;
		padding: 0.45rem 0.75rem;
		border: 1px solid var(--rand);
		border-radius: 8px;
		background: var(--vlak);
		font-size: 0.85rem;
	}

	.label {
		color: var(--gedempt);
		font-size: 0.75rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.score {
		color: var(--gedempt);
		font-variant-numeric: tabular-nums;
	}
</style>
