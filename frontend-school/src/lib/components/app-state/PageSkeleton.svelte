<script lang="ts" module>
	export type PageSkeletonVariant = 'table' | 'cards' | 'form' | 'detail';
</script>

<script lang="ts">
	import { Card, CardContent, CardHeader } from '$lib/components/ui/card';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import TableSkeleton from './TableSkeleton.svelte';

	let {
		variant = 'table',
		rows = 5,
		columns = 4
	}: {
		variant?: PageSkeletonVariant;
		rows?: number;
		columns?: number;
	} = $props();
</script>

{#if variant === 'table'}
	<Card>
		<CardHeader>
			<Skeleton class="h-5 w-40" />
			<Skeleton class="h-4 w-56" />
		</CardHeader>
		<CardContent class="p-0">
			<TableSkeleton {rows} {columns} />
		</CardContent>
	</Card>
{:else if variant === 'cards'}
	<div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
		{#each Array.from({ length: rows }) as _, index (index)}
			<Card>
				<CardContent class="space-y-4 p-4">
					<Skeleton class="h-5 w-2/3" />
					<Skeleton class="h-4 w-full" />
					<Skeleton class="h-4 w-5/6" />
				</CardContent>
			</Card>
		{/each}
	</div>
{:else if variant === 'form'}
	<Card>
		<CardContent class="space-y-5 p-6">
			{#each Array.from({ length: rows }) as _, index (index)}
				<div class="space-y-2">
					<Skeleton class="h-4 w-28" />
					<Skeleton class="h-10 w-full" />
				</div>
			{/each}
		</CardContent>
	</Card>
{:else}
	<Card>
		<CardContent class="space-y-5 p-6">
			<Skeleton class="h-7 w-64" />
			<Skeleton class="h-4 w-full" />
			<Skeleton class="h-4 w-5/6" />
			<div class="grid gap-4 md:grid-cols-3">
				<Skeleton class="h-24 w-full" />
				<Skeleton class="h-24 w-full" />
				<Skeleton class="h-24 w-full" />
			</div>
		</CardContent>
	</Card>
{/if}
