<script lang="ts">
	import { page } from '$app/state';
</script>

<div class="fout">
	<h1>{page.status === 404 ? 'Die pagina is er niet' : 'De app kan niet starten'}</h1>
	<p>{page.error?.message}</p>

	{#if page.error?.herkend}
		<p class="hint">
			De database wordt gemaakt door het programma, niet door de app. Draai op de server één
			keer <code>kaartenjager run</code>, of wijs <code>KAARTENJAGER_DB</code> naar het juiste
			bestand.
		</p>
	{/if}

	<a href="/">terug naar de inbox</a>
</div>

<style>
	.fout {
		max-width: 40rem;
		margin: 4rem auto;
		padding: 0 1rem;
	}

	h1 {
		font-size: 1.3rem;
	}

	.hint {
		color: var(--gedempt, #98a1ae);
		font-size: 0.95rem;
	}

	code {
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
	}
</style>
