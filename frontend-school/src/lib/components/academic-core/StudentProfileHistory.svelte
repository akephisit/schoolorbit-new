<script lang="ts">
	import { untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { loadStudentProfileHistory } from '#lib/academic-core/student-profile-history.js';
	import { PageState, PageSkeleton, RegionUpdatingState } from '#lib/components/app-state/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
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

<section data-testid="student-placement-history" aria-busy={loading}>
	<Card class="relative min-w-0">
		<CardHeader>
			<CardTitle><h2>ประวัติการเรียนและการจัดห้องในปีที่เลือก</h2></CardTitle>
			<CardDescription>การจัดห้องประจำชั้นและช่วงเวลาที่มีผล</CardDescription>
		</CardHeader>
		<CardContent class="min-w-0 space-y-4">
			{#if !allowed}
				<PageState variant="permission" title="ไม่มีสิทธิ์ดูประวัติการจัดห้อง" />
			{:else if loading && !history}
				<PageSkeleton variant="table" rows={3} />
			{:else if error && !history}
				<PageState
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
				<p class="text-sm text-muted-foreground">
					{history.studentYear.gradeLevelName} · {history.studentYear.studyProgramName}
				</p>
				<Table.Root class="min-w-96">
					<Table.Caption class="sr-only">ประวัติห้องประจำชั้น</Table.Caption>
					<Table.Header>
						<Table.Row>
							<Table.Head>ห้อง</Table.Head><Table.Head>เลขที่</Table.Head><Table.Head
								>เริ่มต้น</Table.Head
							><Table.Head>สิ้นสุด</Table.Head><Table.Head>สถานะ</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each history.placements as placement (placement.id)}
							<Table.Row>
								<Table.Cell
									>{history.homerooms.find((room) => room.id === placement.homeroomId)?.name ??
										'ห้องที่ไม่ได้เปิดใช้งาน'}</Table.Cell
								>
								<Table.Cell>{placement.classNumber ?? '—'}</Table.Cell>
								<Table.Cell>{formatCalendarDate(placement.startDate)}</Table.Cell>
								<Table.Cell
									>{placement.endDate ? formatCalendarDate(placement.endDate) : '—'}</Table.Cell
								>
								<Table.Cell>{statusLabels[placement.status]}</Table.Cell>
							</Table.Row>
						{:else}
							<Table.Row
								><Table.Cell colspan={5} class="h-24 text-center text-muted-foreground"
									>ยังไม่มีประวัติการจัดห้องในปีนี้</Table.Cell
								></Table.Row
							>
						{/each}
					</Table.Body>
				</Table.Root>
			{:else}<PageState title="ยังไม่มีข้อมูลนักเรียนในปีที่เลือก" />{/if}
		</CardContent>
	</Card>
</section>
