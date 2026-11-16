<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getLeaveCoverageSettings,
		saveLeaveCoverageSettings,
		type LeaveCoverageSettings
	} from '#lib/api/staff-leave.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	let { source }: { source: Promise<RouteLoadResult<LeaveCoverageSettings | null>> } = $props();
	const region = new LeaveRegion<LeaveCoverageSettings | null>(),
		intent = new LeaveMutationIntent();
	let boundary = $state(''),
		pending = $state(false),
		error = $state(''),
		initialized = false,
		disposed = false;
	const allowed = $derived($can.has(PERMISSIONS.STAFF_LEAVE_MANAGE_SCHOOL));
	$effect.pre(() => region.consume(source));
	$effect.pre(() => {
		const value = region.value;
		if (value)
			untrack(() => {
				if (!initialized) {
					boundary = value.halfDayBoundary?.slice(0, 5) ?? '';
					initialized = true;
				}
			});
	});
	onDestroy(() => {
		disposed = true;
		region.dispose();
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!allowed || !region.value || pending) return;
		pending = true;
		error = '';
		const basis = {
			expectedRevision: region.value.revision,
			halfDayBoundary: boundary ? `${boundary}:00` : null
		};
		try {
			const result = await saveLeaveCoverageSettings({ requestId: intent.for(basis), ...basis });
			if (!disposed) region.patch(result);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'บันทึกไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<section
	class="space-y-3 rounded-xl border bg-card p-4"
	data-testid="leave-coverage-settings"
	aria-busy={region.loading}
>
	<div>
		<h2 class="font-semibold">เวลาครึ่งวันของโรงเรียน</h2>
		<p class="text-sm text-muted-foreground">ใช้แบ่งคาบช่วงเช้าและบ่ายสำหรับคำขอลาครึ่งวัน</p>
	</div>
	{#if region.error}<PageState
			variant="error"
			title="โหลดการตั้งค่าครึ่งวันไม่สำเร็จ"
			description={region.error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => region.retry((signal) => getLeaveCoverageSettings({ signal }))}
		/>{/if}
	{#if region.loading && !region.value}<PageSkeleton variant="form" />{:else if region.value}
		<form onsubmit={save} class="flex flex-wrap items-end gap-3">
			<div class="space-y-2">
				<Label for="coverage-half-day-boundary">เวลาสิ้นสุดช่วงเช้า</Label><Input
					id="coverage-half-day-boundary"
					type="time"
					bind:value={boundary}
					disabled={pending || !allowed}
					class="w-44"
				/>
			</div>
			<LoadingButton type="submit" loading={pending} disabled={!allowed}
				>บันทึกเวลาครึ่งวัน</LoadingButton
			>
		</form>
		{#if !region.value.halfDayBoundary}<p class="text-sm text-muted-foreground">
				ยังไม่กำหนดเวลา คำขอครึ่งวันยังจัดคาบและส่งตามปกติไม่ได้
			</p>{/if}
	{/if}
	{#if error}<PageState
			variant="error"
			title="บันทึกเวลาครึ่งวันไม่สำเร็จ"
			description={error}
			actionLabel="ตรวจสอบข้อมูลล่าสุด"
			onaction={() => region.retry((signal) => getLeaveCoverageSettings({ signal }))}
		/>{/if}
</section>
