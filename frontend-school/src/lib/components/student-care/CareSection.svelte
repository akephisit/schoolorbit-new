<script lang="ts">
	import type { Snippet } from 'svelte';
	import * as Card from '#lib/components/ui/card/index.js';
	let {
		title,
		description,
		children,
		actions
	}: {
		title: string;
		description?: string;
		children: Snippet;
		actions?: Snippet;
	} = $props();
</script>

<Card.Root class="gap-0 py-0">
	<Card.Header class="flex flex-row flex-wrap items-start justify-between gap-3 border-b p-4">
		<div class="min-w-0 space-y-1">
			<h2 class="font-semibold">{title}</h2>
			{#if description}<Card.Description>{description}</Card.Description>{/if}
		</div>
		{#if actions}{@render actions()}{/if}
	</Card.Header>
	<Card.Content class="space-y-4 p-4">{@render children()}</Card.Content>
</Card.Root>
