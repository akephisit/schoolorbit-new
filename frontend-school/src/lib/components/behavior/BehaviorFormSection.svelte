<script lang="ts">
	import type { Component, Snippet } from 'svelte';
	import * as Card from '#lib/components/ui/card/index.js';

	let {
		id,
		title,
		description,
		step,
		icon,
		actions,
		children
	}: {
		id?: string;
		title: string;
		description?: string;
		step?: number;
		icon?: Component;
		actions?: Snippet;
		children: Snippet;
	} = $props();
</script>

<Card.Root {id} class="gap-0 overflow-hidden py-0">
	<Card.Header class="flex flex-row items-start gap-3 border-b p-4 sm:p-5">
		{#if step !== undefined}
			<span
				class="flex size-9 shrink-0 items-center justify-center rounded-full bg-primary/10 text-sm font-semibold text-primary"
				aria-hidden="true">{step}</span
			>
		{:else if icon}
			{@const SectionIcon = icon}
			<span
				class="flex size-9 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary"
				aria-hidden="true"><SectionIcon class="size-4" /></span
			>
		{/if}
		<div class="min-w-0 flex-1 space-y-1">
			<h2 class="font-semibold">{title}</h2>
			{#if description}<Card.Description>{description}</Card.Description>{/if}
		</div>
		{#if actions}<div class="shrink-0">{@render actions()}</div>{/if}
	</Card.Header>
	<Card.Content class="space-y-4 p-4 sm:p-5">
		{@render children()}
	</Card.Content>
</Card.Root>
