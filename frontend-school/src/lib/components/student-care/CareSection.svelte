<script lang="ts">
	import type { Component, Snippet } from 'svelte';
	import * as Card from '#lib/components/ui/card/index.js';
	let {
		title,
		description,
		children,
		actions,
		icon,
		id
	}: {
		title: string;
		description?: string;
		children: Snippet;
		actions?: Snippet;
		icon?: Component;
		id?: string;
	} = $props();
</script>

<Card.Root {id} class="scroll-mt-4 gap-0 py-0">
	<Card.Header class="flex flex-row flex-wrap items-start justify-between gap-3 border-b p-4">
		<div class="min-w-0 space-y-1">
			<h2 class="flex items-center gap-2 font-semibold">
				{#if icon}{@const SectionIcon = icon}<SectionIcon
						class="size-4 shrink-0 text-primary"
						aria-hidden="true"
					/>{/if}{title}
			</h2>
			{#if description}<Card.Description>{description}</Card.Description>{/if}
		</div>
		{#if actions}{@render actions()}{/if}
	</Card.Header>
	<Card.Content class="space-y-4 p-4">{@render children()}</Card.Content>
</Card.Root>
