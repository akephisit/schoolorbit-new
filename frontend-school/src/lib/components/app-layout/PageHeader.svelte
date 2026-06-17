<script lang="ts">
	import type { Component, Snippet } from 'svelte';
	import { ArrowLeft } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import { cn } from '$lib/utils';

	let {
		title,
		description,
		backHref,
		backLabel = 'ย้อนกลับ',
		icon,
		meta,
		actions,
		class: className
	}: {
		title: string;
		description?: string;
		backHref?: string;
		backLabel?: string;
		icon?: Component;
		meta?: Snippet;
		actions?: Snippet;
		class?: string;
	} = $props();
</script>

<header class={cn('flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between', className)}>
	<div class="flex min-w-0 items-start gap-3">
		{#if backHref}
			<Button
				href={backHref}
				variant="ghost"
				size="icon"
				aria-label={backLabel}
				class="mt-0.5 shrink-0"
			>
				<ArrowLeft class="h-4 w-4" />
			</Button>
		{/if}

		{#if icon}
			{@const HeaderIcon = icon}
			<div
				class="mt-0.5 flex h-10 w-10 shrink-0 items-center justify-center rounded-md border bg-card text-muted-foreground"
				aria-hidden="true"
			>
				<HeaderIcon class="h-5 w-5" />
			</div>
		{/if}

		<div class="min-w-0 space-y-1">
			{#if meta}
				<div class="mb-1">
					{@render meta()}
				</div>
			{/if}
			<h1 class="truncate text-2xl font-semibold tracking-tight text-foreground">{title}</h1>
			{#if description}
				<p class="max-w-3xl text-sm text-muted-foreground">{description}</p>
			{/if}
		</div>
	</div>

	{#if actions}
		<div class="flex shrink-0 flex-wrap items-center gap-2 sm:justify-end">
			{@render actions()}
		</div>
	{/if}
</header>
