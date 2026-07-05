<script lang="ts">
	import { toast } from 'svelte-sonner';
	import type { PageProps } from './$types';
	import {
		getAcademicStructure,
		listClassrooms,
		type AcademicStructureData,
		type Classroom
	} from '$lib/api/academic';
	import {
		deleteExamDay,
		generateSeatsForAssignment,
		getExamScheduleWorkspace,
		importExamItems,
		placeExamSession,
		publishExamRound,
		upsertDayRoomAssignment,
		upsertExamDay,
		type ExamRoundStatus,
		type ExamScheduleWorkspace,
		type PlaceExamSessionInput,
		type UpsertDayRoomAssignmentInput,
		type UpsertExamDayInput
	} from '$lib/api/examSchedule';
	import { listRooms, type Room } from '$lib/api/facility';
	import { listStaff, type StaffListItem } from '$lib/api/staff';
	import ExamDaySetupPanel from '$lib/components/academic/exam-schedule/ExamDaySetupPanel.svelte';
	import ExamRoomAssignmentPanel from '$lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte';
	import ExamScheduleTimeline from '$lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte';
	import ReadinessPanel from '$lib/components/academic/exam-schedule/ReadinessPanel.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import * as Tabs from '$lib/components/ui/tabs';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Download, RefreshCw, Send } from 'lucide-svelte';

	let { data }: PageProps = $props();

	let loading = $state(true);
	let refreshing = $state(false);
	let error = $state('');
	let activeTab = $state<'setup' | 'rooms' | 'schedule' | 'review'>('setup');
	let workspace = $state<ExamScheduleWorkspace | null>(null);
	let structure = $state<AcademicStructureData | null>(null);
	let classrooms = $state<Classroom[]>([]);
	let rooms = $state<Room[]>([]);
	let staff = $state<StaffListItem[]>([]);
	let optionsLoading = $state(false);
	let optionsRequested = $state(false);
	let importing = $state(false);
	let publishing = $state(false);
	let savingDay = $state(false);
	let deletingDayId = $state<string | null>(null);
	let savingAssignment = $state(false);
	let generatingAssignmentId = $state<string | null>(null);
	let placingItemId = $state<string | null>(null);
	let requestedRoundId = $state('');
	let loadedRoundId = $state('');
	let workspaceRequestToken = 0;
	let managementOptionsRequestToken = 0;

	const canManageExamSchedules = $derived(
		$can.has(PERMISSIONS.ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)
	);
	const canPublishExamSchedules = $derived(
		$can.has(PERMISSIONS.ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL)
	);
	const pageTitle = $derived(workspace?.round.name ?? data.title);
	const semester = $derived(
		structure?.semesters.find((item) => item.id === workspace?.round.academicSemesterId) ?? null
	);
	const gradeLevels = $derived(structure?.levels ?? []);
	const configuredGradeLevelIds = $derived(
		Array.from(new Set(workspace?.days.flatMap((day) => day.gradeLevelIds) ?? []))
	);

	function resetWorkspaceForRound(roundId: string) {
		workspaceRequestToken += 1;
		managementOptionsRequestToken += 1;
		loadedRoundId = '';
		error = '';
		activeTab = 'setup';
		workspace = null;
		structure = null;
		classrooms = [];
		rooms = [];
		staff = [];
		optionsRequested = false;
		optionsLoading = false;
		importing = false;
		publishing = false;
		savingDay = false;
		deletingDayId = null;
		savingAssignment = false;
		generatingAssignmentId = null;
		placingItemId = null;
		loading = !!roundId;
		refreshing = false;
	}

	async function loadWorkspace(roundId: string, initial = false) {
		const requestToken = ++workspaceRequestToken;
		if (initial) {
			loading = true;
		} else {
			refreshing = true;
		}
		error = '';

		try {
			const [workspaceData, academic] = await Promise.all([
				getExamScheduleWorkspace(roundId),
				getAcademicStructure()
			]);
			if (requestToken !== workspaceRequestToken) return;

			workspace = workspaceData;
			structure = academic.data;
			loadedRoundId = roundId;
		} catch (loadError) {
			if (requestToken !== workspaceRequestToken) return;

			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดพื้นที่จัดตารางสอบได้';
			if (initial) workspace = null;
		} finally {
			if (requestToken === workspaceRequestToken) {
				loading = false;
				refreshing = false;
			}
		}
	}

	async function refreshWorkspace() {
		const roundId = workspace?.round.id ?? loadedRoundId;
		if (!roundId) return;
		await loadWorkspace(roundId, false);
	}

	function isCurrentManagementOptionsRequest(
		requestToken: number,
		roundId: string,
		semesterId: string,
		yearId: string | undefined
	): boolean {
		const currentSemester =
			structure?.semesters.find((item) => item.id === workspace?.round.academicSemesterId) ?? null;

		return (
			requestToken === managementOptionsRequestToken &&
			workspace?.round.id === roundId &&
			workspace.round.academicSemesterId === semesterId &&
			currentSemester?.academic_year_id === yearId
		);
	}

	async function loadManagementOptions() {
		if (!workspace || optionsLoading || optionsRequested) return;

		const requestToken = ++managementOptionsRequestToken;
		const roundId = workspace.round.id;
		const semesterId = workspace.round.academicSemesterId;
		const currentSemester =
			structure?.semesters.find((item) => item.id === workspace?.round.academicSemesterId) ?? null;
		const yearId = currentSemester?.academic_year_id;

		optionsRequested = true;
		optionsLoading = true;
		try {
			const [classroomResponse, roomResponse, staffResponse] = await Promise.all([
				listClassrooms(yearId ? { year_id: yearId } : undefined),
				listRooms(),
				searchStaffOptions('')
			]);
			if (!isCurrentManagementOptionsRequest(requestToken, roundId, semesterId, yearId)) return;

			classrooms = classroomResponse.data;
			rooms = roomResponse.data;
			staff = staffResponse;
		} catch (loadError) {
			if (!isCurrentManagementOptionsRequest(requestToken, roundId, semesterId, yearId)) return;

			optionsRequested = false;
			toast.error(loadError instanceof Error ? loadError.message : 'โหลดตัวเลือกสำหรับจัดห้องสอบไม่สำเร็จ');
		} finally {
			if (!isCurrentManagementOptionsRequest(requestToken, roundId, semesterId, yearId)) return;

			optionsLoading = false;
		}
	}

	async function searchStaffOptions(search: string): Promise<StaffListItem[]> {
		const response = await listStaff({
			status: 'active',
			search: search.trim() || undefined,
			page: 1,
			page_size: 40
		});
		return response.data;
	}

	async function handleImportItems() {
		if (!workspace) return;

		importing = true;
		try {
			const result = await importExamItems(workspace.round.id, {
				gradeLevelIds: configuredGradeLevelIds.length ? configuredGradeLevelIds : undefined
			});
			toast.success(
				`นำเข้า ${result.insertedCount} รายการ ข้ามรายการเดิม ${result.skippedExistingCount}`
			);
			await refreshWorkspace();
		} catch (importError) {
			toast.error(importError instanceof Error ? importError.message : 'นำเข้ารายการสอบไม่สำเร็จ');
		} finally {
			importing = false;
		}
	}

	async function handlePublish() {
		if (!workspace) return;

		publishing = true;
		try {
			const round = await publishExamRound(workspace.round.id);
			workspace = { ...workspace, round };
			toast.success('เผยแพร่ตารางสอบแล้ว');
			await refreshWorkspace();
		} catch (publishError) {
			toast.error(publishError instanceof Error ? publishError.message : 'เผยแพร่ตารางสอบไม่สำเร็จ');
		} finally {
			publishing = false;
		}
	}

	async function handleSaveDay(input: UpsertExamDayInput): Promise<boolean> {
		if (!workspace) return false;

		const roundId = workspace.round.id;
		savingDay = true;
		try {
			await upsertExamDay(roundId, input);
			toast.success('บันทึกวันสอบแล้ว');
			await refreshWorkspace();
			return true;
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'บันทึกวันสอบไม่สำเร็จ');
			return false;
		} finally {
			savingDay = false;
		}
	}

	async function handleDeleteDay(examDayId: string) {
		if (!window.confirm('ลบวันสอบนี้?')) return;

		deletingDayId = examDayId;
		try {
			await deleteExamDay(examDayId);
			toast.success('ลบวันสอบแล้ว');
			await refreshWorkspace();
		} catch (deleteError) {
			toast.error(deleteError instanceof Error ? deleteError.message : 'ลบวันสอบไม่สำเร็จ');
		} finally {
			deletingDayId = null;
		}
	}

	async function handleSaveAssignment(
		examDayId: string,
		input: UpsertDayRoomAssignmentInput
	): Promise<boolean> {
		savingAssignment = true;
		try {
			await upsertDayRoomAssignment(examDayId, input);
			toast.success('บันทึกห้องสอบแล้ว');
			await refreshWorkspace();
			return true;
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'บันทึกห้องสอบไม่สำเร็จ');
			return false;
		} finally {
			savingAssignment = false;
		}
	}

	async function handleGenerateSeats(assignmentId: string) {
		generatingAssignmentId = assignmentId;
		try {
			const seats = await generateSeatsForAssignment(assignmentId, { regenerate: true });
			toast.success(`สร้างเลขที่นั่ง ${seats.length} รายการ`);
			await refreshWorkspace();
		} catch (seatError) {
			toast.error(seatError instanceof Error ? seatError.message : 'สร้างเลขที่นั่งไม่สำเร็จ');
		} finally {
			generatingAssignmentId = null;
		}
	}

	async function handlePlaceExamSession(input: PlaceExamSessionInput): Promise<boolean> {
		placingItemId = input.examScheduleItemId;
		try {
			await placeExamSession({
				examScheduleItemId: input.examScheduleItemId,
				examDayId: input.examDayId,
				startsAt: input.startsAt
			});
			toast.success('บันทึกเวลาสอบแล้ว');
			await refreshWorkspace();
			return true;
		} catch (placeError) {
			toast.error(placeError instanceof Error ? placeError.message : 'บันทึกเวลาสอบไม่สำเร็จ');
			return false;
		} finally {
			placingItemId = null;
		}
	}

	function statusLabel(status: ExamRoundStatus): string {
		return status === 'published' ? 'เผยแพร่แล้ว' : 'ฉบับร่าง';
	}

	function statusVariant(status: ExamRoundStatus): 'default' | 'secondary' {
		return status === 'published' ? 'default' : 'secondary';
	}

	$effect(() => {
		if (canManageExamSchedules && workspace && structure && !optionsRequested && !optionsLoading) {
			loadManagementOptions();
		}
	});

	$effect(() => {
		const roundId = data.roundId;
		if (!roundId || roundId === requestedRoundId) return;

		requestedRoundId = roundId;
		resetWorkspaceForRound(roundId);
		loadWorkspace(roundId, true);
	});
</script>

<svelte:head>
	<title>{pageTitle}</title>
</svelte:head>

<PageShell
	title={pageTitle}
	description={workspace?.round.description ?? semester?.name ?? 'จัดตารางสอบประจำภาคเรียน'}
	backHref="/staff/academic/exam-schedules"
>
	{#snippet meta()}
		{#if workspace}
			<Badge variant={statusVariant(workspace.round.status)}>{statusLabel(workspace.round.status)}</Badge>
		{/if}
	{/snippet}

	{#snippet actions()}
		{#if workspace}
			<div class="flex flex-wrap items-center gap-2">
				<LoadingButton
					variant="outline"
					size="sm"
					loading={refreshing}
					loadingLabel="กำลังโหลด..."
					onclick={refreshWorkspace}
				>
					<RefreshCw class="h-4 w-4" />
					รีเฟรช
				</LoadingButton>
				{#if canManageExamSchedules}
					<LoadingButton
						size="sm"
						variant="outline"
						loading={importing}
						loadingLabel="กำลังนำเข้า..."
						onclick={handleImportItems}
						disabled={workspace.round.status === 'published'}
					>
						<Download class="h-4 w-4" />
						นำเข้า
					</LoadingButton>
				{/if}
				{#if canPublishExamSchedules}
					<LoadingButton
						size="sm"
						loading={publishing}
						loadingLabel="กำลังเผยแพร่..."
						onclick={handlePublish}
						disabled={!workspace.readiness.canPublish || workspace.round.status === 'published'}
					>
						<Send class="h-4 w-4" />
						เผยแพร่
					</LoadingButton>
				{/if}
			</div>
		{/if}
	{/snippet}

	{#if loading}
		<PageSkeleton variant="detail" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดพื้นที่จัดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(data.roundId, true)}
		/>
	{:else if !workspace}
		<PageState title="ไม่พบรอบตารางสอบ" description="รายการที่เปิดอาจถูกลบหรือไม่มีสิทธิ์เข้าถึง" />
	{:else}
		<div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]">
			<div class="min-w-0">
				<Tabs.Root bind:value={activeTab} class="gap-4">
					<Tabs.List class="grid w-full grid-cols-4 md:w-fit">
						<Tabs.Trigger value="setup">Setup</Tabs.Trigger>
						<Tabs.Trigger value="rooms">Rooms</Tabs.Trigger>
						<Tabs.Trigger value="schedule">Schedule</Tabs.Trigger>
						<Tabs.Trigger value="review">Review</Tabs.Trigger>
					</Tabs.List>

					<Tabs.Content value="setup">
						<ExamDaySetupPanel
							days={workspace.days}
							gradeLevels={gradeLevels}
							readonly={!canManageExamSchedules || workspace.round.status === 'published'}
							saving={savingDay}
							deletingDayId={deletingDayId}
							onSaveDay={handleSaveDay}
							onDeleteDay={handleDeleteDay}
						/>
					</Tabs.Content>

					<Tabs.Content value="rooms">
						<ExamRoomAssignmentPanel
							days={workspace.days}
							classrooms={classrooms}
							rooms={rooms}
							staff={staff}
							readonly={!canManageExamSchedules || workspace.round.status === 'published'}
							saving={savingAssignment}
							generatingAssignmentId={generatingAssignmentId}
							onSaveAssignment={handleSaveAssignment}
							onGenerateSeats={handleGenerateSeats}
							onSearchStaff={searchStaffOptions}
						/>
					</Tabs.Content>

					<Tabs.Content value="schedule">
						<ExamScheduleTimeline
							{workspace}
							readonly={!canManageExamSchedules || workspace.round.status === 'published'}
							placingItemId={placingItemId}
							onPlaceSession={handlePlaceExamSession}
						/>
					</Tabs.Content>

					<Tabs.Content value="review">
						<section class="overflow-hidden rounded-md border bg-background">
							<div class="border-b px-4 py-4">
								<h2 class="font-semibold">สรุปก่อนเผยแพร่</h2>
								<p class="text-sm text-muted-foreground">{semester?.name ?? '-'}</p>
							</div>
							<div class="overflow-x-auto">
								<Table class="min-w-[720px]">
									<TableHeader>
										<TableRow>
											<TableHead>วันสอบ</TableHead>
											<TableHead class="text-center">ระดับชั้น</TableHead>
											<TableHead class="text-center">ห้องสอบ</TableHead>
											<TableHead class="text-center">ช่วงปิด</TableHead>
										</TableRow>
									</TableHeader>
									<TableBody>
										{#each workspace.days as day (day.id)}
											<TableRow>
												<TableCell>
													<div class="font-medium">{day.label || day.examDate}</div>
													<div class="font-mono text-xs text-muted-foreground">
														{day.startTime.slice(0, 5)}-{day.endTime.slice(0, 5)}
													</div>
												</TableCell>
												<TableCell class="text-center">
													<Badge variant="outline">{day.gradeLevelIds.length || 'ทั้งหมด'}</Badge>
												</TableCell>
												<TableCell class="text-center">
													<Badge variant="outline">{day.roomAssignments.length}</Badge>
												</TableCell>
												<TableCell class="text-center">
													<Badge variant="outline">{day.blockedWindows.length}</Badge>
												</TableCell>
											</TableRow>
										{/each}
									</TableBody>
								</Table>
							</div>
						</section>
					</Tabs.Content>
				</Tabs.Root>
			</div>

			<aside class="min-w-0 xl:sticky xl:top-20 xl:self-start">
				<ReadinessPanel
					status={workspace.round.status}
					readiness={workspace.readiness}
					days={workspace.days}
					unscheduledItems={workspace.unscheduledItems}
					scheduledSessions={workspace.scheduledSessions}
				/>
			</aside>
		</div>
	{/if}
</PageShell>
