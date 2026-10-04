<script lang="ts">
	import { resolve } from '$app/paths';
	import { ChevronRight } from '@lucide/svelte';
	import { personnelDrilldownHref } from '#lib/forms/staff-personnel.js';
	import type {
		PersonnelBucket,
		PersonnelDimension,
		PersonnelStatusFilter
	} from '#lib/api/personnel.js';
	let {
		title,
		buckets,
		dimension,
		status,
		note
	}: {
		title: string;
		buckets: PersonnelBucket[];
		dimension: PersonnelDimension;
		status: PersonnelStatusFilter;
		note?: string;
	} = $props();
	const maximum = $derived(Math.max(1, ...buckets.map((bucket) => bucket.count)));
</script>

<section class="min-w-0 rounded-xl border bg-card p-5 space-y-4" aria-label={title}>
	<div>
		<h2 class="font-semibold">{title}</h2>
		{#if note}<p class="mt-1 text-xs leading-relaxed text-muted-foreground">{note}</p>{/if}
	</div>
	<ul class="space-y-1">
		{#each buckets as bucket (bucket.key)}<li>
				<a
					href={resolve(
						personnelDrilldownHref(dimension, bucket, status).slice(1) as `staff/manage?${string}`
					)}
					data-sveltekit-preload-data="tap"
					aria-label={`${bucket.label} ${bucket.count} คน`}
					class="group block rounded-lg p-3 text-foreground no-underline transition-colors hover:bg-accent focus-visible:bg-accent focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
					><div class="mb-1.5 flex items-start justify-between gap-3 text-sm">
						<span class="min-w-0 break-words font-medium">{bucket.label}</span>
						<span class="flex shrink-0 items-center gap-2">
							<span class="rounded-md bg-muted px-2 py-0.5 font-semibold tabular-nums">
								{bucket.count.toLocaleString('th-TH')}
								<span class="text-xs font-normal text-muted-foreground">คน</span>
							</span>
							<ChevronRight
								class="size-4 text-muted-foreground group-hover:text-primary group-focus-visible:text-primary"
								aria-hidden="true"
							/>
						</span>
					</div>
					<div class="h-2 rounded-full bg-muted" aria-hidden="true">
						<div
							class="h-full rounded-full bg-primary/75 transition-[width]"
							style:width={`${(bucket.count / maximum) * 100}%`}
						></div>
					</div></a
				>
			</li>{:else}<li class="py-8 text-center text-sm text-muted-foreground">
				ไม่มีบุคลากรในสถานะที่เลือก
			</li>{/each}
	</ul>
</section>
