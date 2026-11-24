<script lang="ts">
	let {
		name,
		color,
		count = null,
		filled = false
	}: { name: string; color: string; count?: number | null; filled?: boolean } = $props();
</script>

<span class="product-label" class:filled style:--product-kleur={color}>
	<span class="stip" aria-hidden="true"></span>
	{name}
	{#if count !== null}<span class="aantal">{count}</span>{/if}
</span>

<style>
	.product-label {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		color: var(--product-kleur);
		white-space: nowrap;
	}

	.filled {
		padding: 0.1rem 0.55rem;
		border-radius: 999px;
		background: color-mix(in srgb, var(--product-kleur) 16%, transparent);
		font-weight: 600;
	}

	.stip {
		width: 0.55rem;
		height: 0.55rem;
		border-radius: 50%;
		background: var(--product-kleur);
		flex: none;
	}

	.aantal {
		font-variant-numeric: tabular-nums;
		color: var(--gedempt);
	}
</style>
