<script lang="ts">
	import type { AcademicPrerequisite } from './prerequisite';
	import * as Alert from '$lib/components/ui/alert';
	import { Button } from '$lib/components/ui/button';
	import { cn } from '$lib/utils';
	import { CircleAlert, CircleDashed } from 'lucide-svelte';

	let {
		prerequisite,
		class: className
	}: {
		prerequisite: AcademicPrerequisite;
		class?: string;
	} = $props();
</script>

<Alert.Root
	class={cn(
		prerequisite.status === 'warning'
			? 'border-amber-500/35 bg-amber-500/[0.06]'
			: 'border-primary/25 bg-primary/[0.035]',
		className
	)}
>
	{#if prerequisite.status === 'warning'}
		<CircleAlert class="size-4 text-amber-600" />
	{:else}
		<CircleDashed class="size-4 text-primary" />
	{/if}
	<Alert.Title>{prerequisite.title}</Alert.Title>
	<Alert.Description class="space-y-3">
		<p>{prerequisite.description}</p>
		{#if prerequisite.actionLabel && prerequisite.href}
			<div class="flex flex-wrap items-center gap-2 pt-1">
				<span class="text-xs font-medium text-foreground">ทางไปต่อ</span>
				<Button href={prerequisite.href} size="sm" variant="outline">
					{prerequisite.actionLabel}
				</Button>
			</div>
		{/if}
	</Alert.Description>
</Alert.Root>
