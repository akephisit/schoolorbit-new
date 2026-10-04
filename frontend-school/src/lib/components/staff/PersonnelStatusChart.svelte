<script lang="ts">
	import { resolve } from '$app/paths';
	import { ChevronRight } from '@lucide/svelte';
	import { personnelDrilldownHref } from '#lib/forms/staff-personnel.js';
	import type { PersonnelBucket } from '#lib/api/personnel.js';
	let { buckets, total }: { buckets: PersonnelBucket[]; total: number } = $props();
	const colors = [
		'var(--chart-1)',
		'var(--chart-2)',
		'var(--chart-3)',
		'var(--chart-4)',
		'var(--chart-5)'
	];
	const segments = $derived.by(() => {
		let offset = 0;
		return buckets.map((bucket, index) => {
			const percentage = total > 0 ? (bucket.count / total) * 100 : 0;
			const segment = { bucket, percentage, offset, color: colors[index % colors.length] };
			offset += percentage;
			return segment;
		});
	});
</script>

<section class="rounded-xl border bg-card p-5 space-y-4" aria-label="สถานะบุคลากรทั้งหมด">
	<h2 class="font-semibold">สถานะบุคลากรทั้งหมด</h2>
	<div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:gap-6">
		<div class="relative h-40 w-40 shrink-0 self-center" aria-hidden="true">
			<svg viewBox="0 0 42 42" class="h-full w-full"
				><circle
					cx="21"
					cy="21"
					r="16"
					fill="none"
					stroke="var(--muted)"
					stroke-width="5"
				/>{#each segments as segment (segment.bucket.key)}{#if segment.percentage > 0}<circle
							cx="21"
							cy="21"
							r="16"
							fill="none"
							stroke={segment.color}
							stroke-width="5"
							pathLength="100"
							stroke-dasharray={`${segment.percentage} ${100 - segment.percentage}`}
							stroke-dashoffset={-segment.offset}
							transform="rotate(-90 21 21)"
						/>{/if}{/each}</svg
			>
			<div class="absolute inset-0 flex flex-col items-center justify-center text-center">
				<span class="text-2xl font-semibold leading-tight tabular-nums"
					>{total.toLocaleString('th-TH')}</span
				>
				<span class="text-xs leading-relaxed">คนทั้งหมด</span>
			</div>
		</div>
		<ul class="w-full min-w-0 flex-1 space-y-1 sm:w-auto">
			{#each segments as segment (segment.bucket.key)}<li>
					<a
						href={resolve(
							personnelDrilldownHref('status', segment.bucket, 'all').slice(
								1
							) as `staff/manage?${string}`
						)}
						data-sveltekit-preload-data="tap"
						class="group flex items-center gap-3 rounded-lg p-3 text-sm text-foreground no-underline transition-colors hover:bg-accent focus-visible:bg-accent focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
						><span
							class="h-2.5 w-2.5 shrink-0 rounded-full"
							style:background={segment.color}
							aria-hidden="true"
						></span><span class="min-w-0 break-words font-medium">{segment.bucket.label}</span><span
							class="ml-auto shrink-0 text-right font-semibold tabular-nums"
							>{segment.bucket.count} คน
							<span class="text-xs font-normal text-muted-foreground"
								>({segment.percentage.toFixed(1)}%)</span
							></span
						>
						<ChevronRight
							class="size-4 shrink-0 text-muted-foreground group-hover:text-primary group-focus-visible:text-primary"
							aria-hidden="true"
						/>
					</a>
				</li>{:else}<li class="text-sm text-muted-foreground">
					ยังไม่มีบุคลากรในขอบเขตที่ดูได้
				</li>{/each}
		</ul>
	</div>
</section>
