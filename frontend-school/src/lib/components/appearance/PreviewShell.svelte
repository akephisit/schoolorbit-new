<script lang="ts">
	import type { Snippet } from 'svelte';
	let {
		title,
		description,
		meta,
		actions,
		children,
		contentClass = ''
	}: {
		title: string;
		description?: string;
		meta?: Snippet;
		actions?: Snippet;
		children?: Snippet;
		contentClass?: string;
	} = $props();
</script>

<div class="school-page-shell">
	<header class="flex flex-wrap justify-between gap-4">
		<div>
			{@render meta?.()}
			<h2 class="text-2xl font-semibold">{title}</h2>
			<p class="text-sm text-muted-foreground">{description}</p>
		</div>
		{@render actions?.()}
	</header>
	<div class={`school-page-content ${contentClass}`}>{@render children?.()}</div>
</div>
