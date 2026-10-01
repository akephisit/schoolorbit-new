<script lang="ts">
	import { resolve } from '$app/paths';
	import { personnelDrilldownHref } from '$lib/forms/staff-personnel';
	import type { PersonnelBucket } from '$lib/api/personnel';
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
	<div class="flex flex-wrap items-center gap-6">
		<svg viewBox="0 0 42 42" class="h-40 w-40 shrink-0" aria-hidden="true"
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
					/>{/if}{/each}<text
				x="21"
				y="21"
				dominant-baseline="central"
				text-anchor="middle"
				fill="currentColor"
				font-size="6"
				font-weight="600">{total.toLocaleString('th-TH')}</text
			><text x="21" y="28" text-anchor="middle" fill="currentColor" font-size="3">คนทั้งหมด</text
			></svg
		>
		<ul class="min-w-0 flex-1 space-y-3">
			{#each segments as segment (segment.bucket.key)}<li>
					<a
						href={resolve(
							personnelDrilldownHref('status', segment.bucket, 'all') as '/staff/manage'
						)}
						data-sveltekit-preload-data="tap"
						class="flex items-center gap-3 rounded text-sm hover:text-primary focus-visible:outline-2 focus-visible:outline-ring"
						><span
							class="h-2.5 w-2.5 shrink-0 rounded-full"
							style:background={segment.color}
							aria-hidden="true"
						></span><span class="min-w-0 break-words">{segment.bucket.label}</span><span
							class="ml-auto shrink-0 tabular-nums"
							>{segment.bucket.count} คน
							<span class="text-xs text-muted-foreground">({segment.percentage.toFixed(1)}%)</span
							></span
						></a
					>
				</li>{:else}<li class="text-sm text-muted-foreground">
					ยังไม่มีบุคลากรในขอบเขตที่ดูได้
				</li>{/each}
		</ul>
	</div>
</section>
