<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		listAssignedLeaveCoverage,
		type LeaveCoverageTask,
		type LeaveCoverageTaskPage
	} from '#lib/api/staff-leave.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import CoverageTask from './CoverageTask.svelte';
	let {
		source,
		offset
	}: { source: Promise<RouteLoadResult<LeaveCoverageTaskPage | null>>; offset: number } = $props();
	const region = new LeaveRegion<LeaveCoverageTaskPage | null>();
	$effect.pre(() => region.consume(source));
	onDestroy(() => region.dispose());
	function retry() {
		region.retry((signal) => listAssignedLeaveCoverage({ signal, query: { limit: 20, offset } }));
	}
	function patch(task: LeaveCoverageTask) {
		if (region.value)
			region.patch({
				...region.value,
				items: region.value.items.map((value) =>
					value.requestId === task.requestId ? task : value
				)
			});
	}
</script>

<PageShell
	title="คาบสอนที่มอบหมายให้ฉัน"
	description="ตรวจสอบวิชาที่สอนจริงและคาบคืนก่อนตอบรับ"
	backHref="/staff/leave"
	contentClass="space-y-4"
>
	<section aria-busy={region.loading} class="space-y-4" data-testid="leave-assigned-tasks">
		{#if region.error}<PageState
				variant="error"
				title="โหลดคาบสอนที่มอบหมายไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={retry}
			/>{/if}
		{#if region.loading && !region.value}<PageSkeleton
				variant="cards"
			/>{:else if region.value?.items.length}
			{#each region.value.items as task (task.requestId)}{#key `${task.planRevision}|${task.planHash}`}<CoverageTask
						{task}
						onpatched={patch}
						onrefresh={retry}
					/>{/key}{/each}
			<div class="flex justify-end gap-2">
				<Button
					variant="outline"
					disabled={!offset}
					href={offset ? `/staff/leave/assigned?offset=${Math.max(0, offset - 20)}` : undefined}
					>ก่อนหน้า</Button
				><Button
					variant="outline"
					disabled={!region.value.hasMore}
					href={region.value.hasMore ? `/staff/leave/assigned?offset=${offset + 20}` : undefined}
					>ถัดไป</Button
				>
			</div>
		{:else if !region.error}<PageState title="ยังไม่มีคาบสอนที่มอบหมายให้คุณ" />{/if}
	</section>
</PageShell>
