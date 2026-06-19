<script lang="ts" module>
	export type PageStateVariant = 'empty' | 'error' | 'permission';
</script>

<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Alert, AlertDescription, AlertTitle } from '$lib/components/ui/alert';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardContent } from '$lib/components/ui/card';
	import { cn } from '$lib/utils';
	import { AlertTriangle, Inbox, ShieldAlert } from 'lucide-svelte';

	let {
		variant = 'empty',
		title,
		description,
		actionLabel,
		href,
		onaction,
		action,
		class: className
	}: {
		variant?: PageStateVariant;
		title: string;
		description?: string;
		actionLabel?: string;
		href?: string;
		onaction?: () => void;
		action?: Snippet;
		class?: string;
	} = $props();
</script>

{#if variant === 'error'}
	<div class={cn('space-y-3', className)}>
		<Alert variant="destructive">
			<AlertTriangle class="h-4 w-4" />
			<AlertTitle>{title}</AlertTitle>
			{#if description}
				<AlertDescription>{description}</AlertDescription>
			{/if}
		</Alert>
		{#if action}
			{@render action()}
		{:else if actionLabel}
			<Button variant="outline" onclick={onaction} {href}>{actionLabel}</Button>
		{/if}
	</div>
{:else if variant === 'permission'}
	<Alert class={className}>
		<ShieldAlert class="h-4 w-4" />
		<AlertTitle>{title}</AlertTitle>
		{#if description}
			<AlertDescription>{description}</AlertDescription>
		{/if}
	</Alert>
{:else}
	<Card class={className}>
		<CardContent class="flex min-h-48 flex-col items-center justify-center p-8 text-center">
			<Inbox class="text-muted-foreground mb-4 h-12 w-12" />
			<h2 class="text-foreground text-lg font-medium">{title}</h2>
			{#if description}
				<p class="text-muted-foreground mt-2 max-w-md text-sm">{description}</p>
			{/if}
			{#if action}
				<div class="mt-4">
					{@render action()}
				</div>
			{:else if actionLabel}
				<Button class="mt-4" onclick={onaction} {href}>{actionLabel}</Button>
			{/if}
		</CardContent>
	</Card>
{/if}
