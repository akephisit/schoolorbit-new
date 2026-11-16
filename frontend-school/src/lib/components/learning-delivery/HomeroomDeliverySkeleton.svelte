<script lang="ts">
	import { Skeleton } from '#lib/components/ui/skeleton/index.js';
	import { TableSkeleton } from '#lib/components/app-state/index.js';

	let { canManage = false }: { canManage?: boolean } = $props();
</script>

<div
	class="space-y-4"
	role="status"
	aria-label="กำลังโหลดภาพรวมรายห้อง"
	data-testid="delivery-homerooms-skeleton"
>
	<div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-hidden="true">
		{#each Array.from({ length: 4 }) as _, index (index)}
			<div class="rounded-xl border bg-card p-4 shadow-sm" data-delivery-metric>
				<Skeleton class="h-4 w-28 max-w-full" />
				<Skeleton class="mt-2 h-8 w-16" />
				<Skeleton class="h-4 w-40 max-w-full" />
			</div>
		{/each}
	</div>
	<div class="overflow-hidden rounded-2xl border bg-card shadow-sm" aria-hidden="true">
		<div class="border-b bg-muted/20 p-4">
			{#if canManage}
				<div class="mb-3 flex flex-wrap gap-2">
					<Skeleton class="h-8 w-36" />
					<Skeleton class="h-8 w-36" />
					<Skeleton class="h-8 w-48" />
				</div>
			{/if}
			<div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
				<div class="min-w-0 space-y-2">
					<Skeleton class="h-5 w-48 max-w-full" />
					<Skeleton class="h-4 w-96 max-w-full" />
				</div>
				<div class="grid shrink-0 gap-2 sm:grid-cols-[minmax(250px,1fr)_190px]">
					<Skeleton class="h-9 w-full" />
					<Skeleton class="h-9 w-full" />
				</div>
			</div>
		</div>
		{#each Array.from({ length: 3 }) as _, index (index)}
			<div class="flex items-center justify-between gap-3 border-b p-4 last:border-b-0">
				<div class="flex min-w-0 items-center gap-3">
					<Skeleton class="size-10 shrink-0 rounded-xl" />
					<div class="min-w-0 space-y-2">
						<Skeleton class="h-5 w-28 max-w-full" />
						<Skeleton class="h-4 w-44 max-w-full" />
					</div>
				</div>
				<Skeleton class="h-8 w-20 shrink-0" />
			</div>
			{#if index === 0}
				<div class="border-b bg-background">
					<div class="flex flex-wrap items-center justify-between gap-3 border-b px-4 py-3">
						<div class="min-w-0 space-y-2">
							<Skeleton class="h-4 w-40 max-w-full" />
							<Skeleton class="h-3 w-72 max-w-full" />
						</div>
						<Skeleton class="h-8 w-32" />
					</div>
					<div class="overflow-x-auto">
						<TableSkeleton rows={3} columns={6} />
					</div>
				</div>
			{/if}
		{/each}
	</div>
</div>
