<script lang="ts">
	import { untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { loadStudentProfileHistory } from '#lib/academic-core/student-profile-history.js';
	import { PageState, PageSkeleton, RegionUpdatingState } from '#lib/components/app-state/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';
	let {
		source,
		profileKey,
		studentId,
		academicYearId
	}: {
		source: Promise<RouteLoadResult<Awaited<ReturnType<typeof loadStudentProfileHistory>> | null>>;
		profileKey: string;
		studentId: string;
		academicYearId: string;
	} = $props();
	let history = $state.raw<Awaited<ReturnType<typeof loadStudentProfileHistory>> | null>(null);
	let loading = $state(true),
		error = $state('');
	let owner = '';
	const request = new LatestRequest();
	const allowed = $derived(
		$can.has(PERMISSIONS.STUDENT_ACADEMIC_YEAR_READ_SCHOOL) &&
			$can.hasAny(PERMISSIONS.HOMEROOM_READ_SCHOOL, PERMISSIONS.HOMEROOM_MANAGE_SCHOOL)
	);
	function apply(result: Awaited<typeof source>, revision: number) {
		if (!request.isCurrent(revision)) return;
		loading = false;
		if (result.ok) history = result.data;
		else error = result.error;
	}
	$effect.pre(() => {
		const read = source,
			key = profileKey;
		untrack(() => {
			if (owner !== key) {
				owner = key;
				history = null;
			}
			loading = true;
			error = '';
			const ticket = request.begin();
			void read.then((result) => apply(result, ticket.revision));
		});
		return () => request.abort();
	});
	async function retry() {
		if (!allowed) return;
		const ticket = request.begin();
		loading = true;
		error = '';
		apply(
			await captureRouteLoad(
				loadStudentProfileHistory(studentId, academicYearId, { signal: ticket.signal }),
				'โหลดประวัติการจัดห้องไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	const statusLabels = {
		planned: 'เตรียมการ',
		current: 'ห้องปัจจุบัน',
		ended: 'สิ้นสุดแล้ว',
		cancelled: 'ยกเลิก'
	};
</script>

<section
	class="relative rounded-xl border bg-card p-4 sm:p-6"
	data-testid="student-placement-history"
	aria-busy={loading}
>
	<h2 class="mb-4 text-xl font-semibold">ประวัติการเรียนและการจัดห้องในปีที่เลือก</h2>
	{#if !allowed}<PageState variant="permission" title="ไม่มีสิทธิ์ดูประวัติการจัดห้อง" />
	{:else if loading && !history}<PageSkeleton variant="table" rows={3} />
	{:else if error && !history}<PageState
			variant="error"
			title="โหลดประวัติการจัดห้องไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={retry}
		/>
	{:else if history?.studentYear}
		{#if loading}<RegionUpdatingState label="กำลังอัปเดตประวัติการจัดห้อง" />{/if}
		{#if error}<PageState
				variant="error"
				title="อัปเดตประวัติการจัดห้องไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={retry}
			/>{/if}
		<p class="mb-4 text-sm text-muted-foreground">
			{history.studentYear.gradeLevelName} · {history.studentYear.studyProgramName}
		</p>
		<div class="overflow-x-auto">
			<table class="w-full min-w-96 text-left text-sm">
				<caption class="sr-only">ประวัติห้องประจำชั้น</caption><thead
					><tr class="border-b"
						><th class="p-2">ห้อง</th><th class="p-2">เลขที่</th><th class="p-2">เริ่มต้น</th><th
							class="p-2">สิ้นสุด</th
						><th class="p-2">สถานะ</th></tr
					></thead
				><tbody>
					{#each history.placements as placement (placement.id)}<tr class="border-b"
							><td class="p-2"
								>{history.homerooms.find((room) => room.id === placement.homeroomId)?.name ??
									'ห้องที่ไม่ได้เปิดใช้งาน'}</td
							><td class="p-2">{placement.classNumber ?? '—'}</td><td class="p-2"
								>{formatCalendarDate(placement.startDate)}</td
							><td class="p-2">{placement.endDate ? formatCalendarDate(placement.endDate) : '—'}</td
							><td class="p-2">{statusLabels[placement.status]}</td></tr
						>{:else}<tr
							><td colspan="5" class="p-4 text-muted-foreground"
								>ยังไม่มีประวัติการจัดห้องในปีนี้</td
							></tr
						>{/each}
				</tbody>
			</table>
		</div>
	{:else}<PageState title="ยังไม่มีข้อมูลนักเรียนในปีที่เลือก" />{/if}
</section>
