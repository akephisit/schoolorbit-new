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
		clearMismatchedExamItems,
		deleteExamDay,
		deleteExamSession,
		generateSeatsForAssignment,
		getExamInvigilatorWorkspace,
		getExamScheduleWorkspace,
		importExamItems,
		listExamInvigilatorStaffOptions,
		placeExamSession,
		publishExamRound,
		updateExamRound,
		updateExamAssignmentInvigilators,
		upsertDayRoomAssignment,
		upsertExamDay,
		type ExamInvigilatorStaffOption,
		type ExamInvigilatorWorkspace,
		type ExamRoundKind,
		type ExamRoundStatus,
		type ExamScheduleWorkspace,
		type PlaceExamSessionInput,
		type UpsertDayRoomAssignmentInput,
		type UpsertExamDayInput
	} from '$lib/api/examSchedule';
	import { listRooms, type Room } from '$lib/api/facility';
	import CompactExamScheduleStatus from '$lib/components/academic/exam-schedule/CompactExamScheduleStatus.svelte';
	import ExamDaySetupPanel from '$lib/components/academic/exam-schedule/ExamDaySetupPanel.svelte';
	import ExamInvigilatorPanel from '$lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte';
	import ExamRoomAssignmentPanel from '$lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte';
	import ExamScheduleTimeline from '$lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte';
	import MobileDragDropPolyfill from '$lib/components/MobileDragDropPolyfill.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Badge } from '$lib/components/ui/badge';
	import * as Select from '$lib/components/ui/select';
	import * as Tabs from '$lib/components/ui/tabs';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Download, RefreshCw, Send, Trash2 } from 'lucide-svelte';

	let { data }: PageProps = $props();

	let loading = $state(true);
	let refreshing = $state(false);
	let error = $state('');
	let activeTab = $state<'setup' | 'rooms' | 'schedule' | 'invigilators'>('setup');
	let workspace = $state<ExamScheduleWorkspace | null>(null);
	let structure = $state<AcademicStructureData | null>(null);
	let classrooms = $state<Classroom[]>([]);
	let rooms = $state<Room[]>([]);
	let staff = $state<ExamInvigilatorStaffOption[]>([]);
	let invigilatorWorkspace = $state<ExamInvigilatorWorkspace | null>(null);
	let loadingInvigilators = $state(false);
	let invigilatorLoadError = $state('');
	let savingInvigilatorAssignmentId = $state<string | null>(null);
	let staffLoading = $state(false);
	let staffRequested = $state(false);
	let optionsLoading = $state(false);
	let optionsRequested = $state(false);
	let importing = $state(false);
	let clearingMismatchedItems = $state(false);
	let publishing = $state(false);
	let savingDay = $state(false);
	let deletingDayId = $state<string | null>(null);
	let savingAssignment = $state(false);
	let generatingAssignmentId = $state<string | null>(null);
	let placingItemId = $state<string | null>(null);
	let unschedulingSessionId = $state<string | null>(null);
	let requestedRoundId = $state('');
	let loadedRoundId = $state('');
	let workspaceRequestToken = 0;
	let managementOptionsRequestToken = 0;
	let invigilatorWorkspaceRequestToken = 0;
	let staffOptionsRequestToken = 0;
	let savingRoundKind = $state(false);
	let examKindDialogOpen = $state(false);
	let pendingExamKind = $state<ExamRoundKind | null>(null);
	let clearMismatchedDialogOpen = $state(false);

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
	const examScheduleItemCount = $derived(
		(workspace?.unscheduledItems.length ?? 0) + (workspace?.scheduledSessions.length ?? 0)
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
		invigilatorWorkspace = null;
		loadingInvigilators = false;
		invigilatorLoadError = '';
		savingInvigilatorAssignmentId = null;
		staffLoading = false;
		staffRequested = false;
		invigilatorWorkspaceRequestToken += 1;
		staffOptionsRequestToken += 1;
		optionsRequested = false;
		optionsLoading = false;
		importing = false;
		clearingMismatchedItems = false;
		publishing = false;
		savingDay = false;
		deletingDayId = null;
		savingAssignment = false;
		generatingAssignmentId = null;
		placingItemId = null;
		unschedulingSessionId = null;
		savingRoundKind = false;
		examKindDialogOpen = false;
		pendingExamKind = null;
		clearMismatchedDialogOpen = false;
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

	async function refreshWorkspace(refreshInvigilators = false) {
		const roundId = workspace?.round.id ?? loadedRoundId;
		if (!roundId) return;
		const shouldRefreshInvigilators =
			refreshInvigilators || invigilatorWorkspace !== null || activeTab === 'invigilators';

		await loadWorkspace(roundId, false);
		if (shouldRefreshInvigilators) {
			await refreshOrInvalidateInvigilators(roundId);
		}
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
			const [classroomResponse, roomResponse] = await Promise.all([
				listClassrooms(yearId ? { year_id: yearId } : undefined),
				listRooms()
			]);
			if (!isCurrentManagementOptionsRequest(requestToken, roundId, semesterId, yearId)) return;

			classrooms = classroomResponse.data;
			rooms = roomResponse.data;
		} catch (loadError) {
			if (!isCurrentManagementOptionsRequest(requestToken, roundId, semesterId, yearId)) return;

			optionsRequested = false;
			toast.error(
				loadError instanceof Error ? loadError.message : 'โหลดตัวเลือกสำหรับจัดห้องสอบไม่สำเร็จ'
			);
		} finally {
			if (!isCurrentManagementOptionsRequest(requestToken, roundId, semesterId, yearId)) return;

			optionsLoading = false;
		}
	}

	function isCurrentStaffOptionsRequest(requestToken: number, roundId: string): boolean {
		return requestToken === staffOptionsRequestToken && workspace?.round.id === roundId;
	}

	async function loadInvigilatorStaffOptions() {
		const roundId = workspace?.round.id ?? loadedRoundId;
		if (!roundId || staffLoading || staffRequested) return;

		const requestToken = ++staffOptionsRequestToken;
		staffRequested = true;
		staffLoading = true;
		try {
			const staffOptions = await listExamInvigilatorStaffOptions(roundId, { limit: 40 });
			if (!isCurrentStaffOptionsRequest(requestToken, roundId)) return;

			staff = staffOptions;
		} catch (loadError) {
			if (!isCurrentStaffOptionsRequest(requestToken, roundId)) return;

			toast.error(
				loadError instanceof Error ? loadError.message : 'โหลดรายชื่อครูสำหรับจัดกรรมการไม่สำเร็จ'
			);
		} finally {
			if (isCurrentStaffOptionsRequest(requestToken, roundId)) {
				staffLoading = false;
			}
		}
	}

	async function searchStaffOptions(search: string): Promise<ExamInvigilatorStaffOption[]> {
		const roundId = workspace?.round.id ?? loadedRoundId;
		if (!roundId) return [];

		return listExamInvigilatorStaffOptions(roundId, {
			search: search.trim() || undefined,
			limit: 40
		});
	}

	async function loadInvigilators(roundId = workspace?.round.id ?? loadedRoundId) {
		if (!roundId) return;

		const requestToken = ++invigilatorWorkspaceRequestToken;
		loadingInvigilators = true;
		invigilatorLoadError = '';
		try {
			const invigilatorData = await getExamInvigilatorWorkspace(roundId);
			if (requestToken !== invigilatorWorkspaceRequestToken) return;

			invigilatorWorkspace = invigilatorData;
		} catch (loadError) {
			if (requestToken !== invigilatorWorkspaceRequestToken) return;

			invigilatorWorkspace = null;
			invigilatorLoadError =
				loadError instanceof Error ? loadError.message : 'โหลดข้อมูลกรรมการคุมสอบไม่สำเร็จ';
			toast.error(invigilatorLoadError);
		} finally {
			if (requestToken === invigilatorWorkspaceRequestToken) {
				loadingInvigilators = false;
			}
		}
	}

	async function refreshOrInvalidateInvigilators(roundId = workspace?.round.id ?? loadedRoundId) {
		if (!roundId) return;

		invigilatorLoadError = '';
		if (activeTab === 'invigilators') {
			await loadInvigilators(roundId);
		} else {
			invigilatorWorkspaceRequestToken += 1;
			loadingInvigilators = false;
			invigilatorWorkspace = null;
		}
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
			await refreshWorkspace(true);
		} catch (importError) {
			toast.error(importError instanceof Error ? importError.message : 'นำเข้ารายการสอบไม่สำเร็จ');
		} finally {
			importing = false;
		}
	}

	function handleClearMismatchedItems() {
		if (!workspace) return;
		clearMismatchedDialogOpen = true;
	}

	async function confirmClearMismatchedItems() {
		if (!workspace) return;

		clearMismatchedDialogOpen = false;
		clearingMismatchedItems = true;
		try {
			const result = await clearMismatchedExamItems(workspace.round.id);
			toast.success(`ล้างรายการไม่ตรงรอบสอบ ${result.deletedCount} รายการ`);
			await refreshWorkspace(true);
		} catch (clearError) {
			toast.error(
				clearError instanceof Error ? clearError.message : 'ล้างรายการสอบที่ไม่ตรงรอบสอบไม่สำเร็จ'
			);
		} finally {
			clearingMismatchedItems = false;
		}
	}

	function isExamRoundKind(value: string): value is ExamRoundKind {
		return value === 'midterm' || value === 'final';
	}

	async function handleUpdateExamKind(value: string) {
		if (
			!workspace ||
			!isExamRoundKind(value) ||
			value === workspace.round.examKind ||
			workspace.round.status === 'published'
		) {
			return;
		}

		if (examScheduleItemCount > 0) {
			pendingExamKind = value;
			examKindDialogOpen = true;
			return;
		}

		await saveExamKind(value);
	}

	async function saveExamKind(value: ExamRoundKind) {
		if (!workspace) return;

		savingRoundKind = true;
		try {
			const round = await updateExamRound(workspace.round.id, { examKind: value });
			workspace = { ...workspace, round };
			toast.success(`เปลี่ยนชนิดรอบสอบเป็น${examRoundKindLabel(round.examKind)}แล้ว`);
		} catch (updateError) {
			toast.error(updateError instanceof Error ? updateError.message : 'บันทึกชนิดรอบสอบไม่สำเร็จ');
		} finally {
			savingRoundKind = false;
		}
	}

	async function confirmExamKindChange() {
		const nextKind = pendingExamKind;
		if (!nextKind) return;

		examKindDialogOpen = false;
		pendingExamKind = null;
		await saveExamKind(nextKind);
	}

	function cancelExamKindChange() {
		pendingExamKind = null;
	}

	async function handlePublish() {
		if (!workspace) return;

		publishing = true;
		try {
			const round = await publishExamRound(workspace.round.id);
			workspace = { ...workspace, round };
			toast.success('เผยแพร่ตารางสอบแล้ว');
			await refreshWorkspace(true);
		} catch (publishError) {
			toast.error(
				publishError instanceof Error ? publishError.message : 'เผยแพร่ตารางสอบไม่สำเร็จ'
			);
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
			await refreshWorkspace(true);
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
			await refreshWorkspace(true);
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
			await refreshWorkspace(true);
			return true;
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'บันทึกห้องสอบไม่สำเร็จ');
			return false;
		} finally {
			savingAssignment = false;
		}
	}

	async function handleSaveInvigilators(
		assignmentId: string,
		staffIds: string[]
	): Promise<boolean> {
		const roundId = workspace?.round.id ?? loadedRoundId;
		if (!roundId) return false;

		savingInvigilatorAssignmentId = assignmentId;
		try {
			await updateExamAssignmentInvigilators(assignmentId, { invigilatorStaffIds: staffIds });
			toast.success('บันทึกกรรมการคุมสอบแล้ว');
			await Promise.all([loadWorkspace(roundId, false), loadInvigilators(roundId)]);
			return true;
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'บันทึกกรรมการคุมสอบไม่สำเร็จ');
			return false;
		} finally {
			savingInvigilatorAssignmentId = null;
		}
	}

	async function handleGenerateSeats(assignmentId: string) {
		generatingAssignmentId = assignmentId;
		try {
			const seats = await generateSeatsForAssignment(assignmentId, { regenerate: true });
			toast.success(`สร้างเลขที่นั่ง ${seats.length} รายการ`);
			await refreshWorkspace(true);
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
			await refreshWorkspace(true);
			return true;
		} catch (placeError) {
			toast.error(placeError instanceof Error ? placeError.message : 'บันทึกเวลาสอบไม่สำเร็จ');
			return false;
		} finally {
			placingItemId = null;
		}
	}

	async function handleUnscheduleExamSession(sessionId: string): Promise<boolean> {
		const session = workspace?.scheduledSessions.find((item) => item.id === sessionId);
		if (
			!session ||
			!workspace ||
			!canManageExamSchedules ||
			workspace.round.status === 'published'
		) {
			return false;
		}

		unschedulingSessionId = sessionId;
		try {
			await deleteExamSession(sessionId);
			toast.success('เอารายการสอบออกจากตารางแล้ว');
			await refreshWorkspace(true);
			return true;
		} catch (deleteError) {
			toast.error(
				deleteError instanceof Error ? deleteError.message : 'เอารายการสอบออกจากตารางไม่สำเร็จ'
			);
			return false;
		} finally {
			unschedulingSessionId = null;
		}
	}

	function statusLabel(status: ExamRoundStatus): string {
		return status === 'published' ? 'เผยแพร่แล้ว' : 'ฉบับร่าง';
	}

	function statusVariant(status: ExamRoundStatus): 'default' | 'secondary' {
		return status === 'published' ? 'default' : 'secondary';
	}

	function examRoundKindLabel(kind: ExamRoundKind): string {
		return kind === 'final' ? 'ปลายภาค' : 'กลางภาค';
	}

	$effect(() => {
		if (canManageExamSchedules && workspace && structure && !optionsRequested && !optionsLoading) {
			loadManagementOptions();
		}
	});

	$effect(() => {
		const roundId = workspace?.round.id ?? loadedRoundId;
		if (
			activeTab === 'invigilators' &&
			roundId &&
			invigilatorWorkspace === null &&
			!loadingInvigilators &&
			!invigilatorLoadError
		) {
			loadInvigilators(roundId);
		}
	});

	$effect(() => {
		if (
			activeTab === 'invigilators' &&
			canManageExamSchedules &&
			workspace?.round.status !== 'published' &&
			!staffRequested &&
			!staffLoading
		) {
			loadInvigilatorStaffOptions();
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

<MobileDragDropPolyfill />

<PageShell
	title={pageTitle}
	description={workspace?.round.description ?? semester?.name ?? 'จัดตารางสอบประจำภาคเรียน'}
	backHref="/staff/academic/exam-schedules"
>
	{#snippet meta()}
		{#if workspace}
			<Badge variant="outline">{examRoundKindLabel(workspace.round.examKind)}</Badge>
			<Badge variant={statusVariant(workspace.round.status)}
				>{statusLabel(workspace.round.status)}</Badge
			>
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
					onclick={() => refreshWorkspace(true)}
				>
					<RefreshCw class="h-4 w-4" />
					รีเฟรช
				</LoadingButton>
				<CompactExamScheduleStatus
					status={workspace.round.status}
					readiness={workspace.readiness}
					days={workspace.days}
					unscheduledItems={workspace.unscheduledItems}
					scheduledSessions={workspace.scheduledSessions}
					invigilatorAssignedCount={invigilatorWorkspace?.assignments.filter(
						(assignment) => assignment.invigilators.length > 0
					).length ?? undefined}
					invigilatorAssignmentCount={invigilatorWorkspace?.assignments.length ?? undefined}
				/>
				{#if canManageExamSchedules}
					<Select.Root
						type="single"
						value={workspace.round.examKind}
						onValueChange={handleUpdateExamKind}
					>
						<Select.Trigger
							class="h-9 w-36"
							disabled={workspace.round.status === 'published' || savingRoundKind}
						>
							{savingRoundKind ? 'กำลังบันทึก...' : examRoundKindLabel(workspace.round.examKind)}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="midterm">กลางภาค</Select.Item>
							<Select.Item value="final">ปลายภาค</Select.Item>
						</Select.Content>
					</Select.Root>
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
		<div class="space-y-4">
			<Tabs.Root bind:value={activeTab} class="gap-4">
				<Tabs.List class="grid w-full grid-cols-4 md:w-fit">
					<Tabs.Trigger value="setup">Setup</Tabs.Trigger>
					<Tabs.Trigger value="rooms">Rooms</Tabs.Trigger>
					<Tabs.Trigger value="schedule">Schedule</Tabs.Trigger>
					<Tabs.Trigger value="invigilators">กรรมการ</Tabs.Trigger>
				</Tabs.List>

				<Tabs.Content value="setup">
					<ExamDaySetupPanel
						days={workspace.days}
						{gradeLevels}
						readonly={!canManageExamSchedules || workspace.round.status === 'published'}
						saving={savingDay}
						{deletingDayId}
						onSaveDay={handleSaveDay}
						onDeleteDay={handleDeleteDay}
					/>
				</Tabs.Content>

				<Tabs.Content value="rooms">
					<ExamRoomAssignmentPanel
						days={workspace.days}
						{classrooms}
						{rooms}
						readonly={!canManageExamSchedules || workspace.round.status === 'published'}
						saving={savingAssignment}
						{generatingAssignmentId}
						onSaveAssignment={handleSaveAssignment}
						onGenerateSeats={handleGenerateSeats}
					/>
				</Tabs.Content>

				<Tabs.Content value="schedule">
					{#if canManageExamSchedules && workspace.round.status !== 'published'}
						<div
							class="mb-3 flex flex-col gap-3 rounded-md border bg-background p-3 md:flex-row md:items-center md:justify-between"
						>
							<div class="min-w-0">
								<p class="text-sm font-medium text-foreground">
									รายการสอบสำหรับ{examRoundKindLabel(workspace.round.examKind)}
								</p>
								<p class="text-xs text-muted-foreground">
									นำเข้ารายการตามชนิดรอบสอบ หรือล้างรายการที่หลุดมาจากรอบอื่น
								</p>
							</div>
							<div class="flex flex-wrap items-center gap-2">
								<LoadingButton
									size="sm"
									variant="outline"
									loading={importing}
									loadingLabel="กำลังนำเข้า..."
									onclick={handleImportItems}
									disabled={clearingMismatchedItems ||
										placingItemId !== null ||
										unschedulingSessionId !== null}
								>
									<Download class="h-4 w-4" />
									นำเข้าเฉพาะ {examRoundKindLabel(workspace.round.examKind)}
								</LoadingButton>
								<LoadingButton
									size="sm"
									variant="destructive"
									loading={clearingMismatchedItems}
									loadingLabel="กำลังล้าง..."
									onclick={handleClearMismatchedItems}
									disabled={importing || placingItemId !== null || unschedulingSessionId !== null}
								>
									<Trash2 class="h-4 w-4" />
									ล้างรายการไม่ตรงรอบสอบ
								</LoadingButton>
							</div>
						</div>
					{/if}
					<ExamScheduleTimeline
						{workspace}
						readonly={!canManageExamSchedules || workspace.round.status === 'published'}
						{placingItemId}
						{unschedulingSessionId}
						onPlaceSession={handlePlaceExamSession}
						onUnscheduleSession={handleUnscheduleExamSession}
					/>
				</Tabs.Content>

				<Tabs.Content value="invigilators">
					<ExamInvigilatorPanel
						days={workspace.days}
						workspace={invigilatorWorkspace}
						{staff}
						loading={loadingInvigilators}
						loadError={invigilatorLoadError}
						readonly={!canManageExamSchedules || workspace.round.status === 'published'}
						savingAssignmentId={savingInvigilatorAssignmentId}
						onSaveInvigilators={handleSaveInvigilators}
						onSearchStaff={searchStaffOptions}
						onRetry={() => loadInvigilators()}
					/>
				</Tabs.Content>
			</Tabs.Root>

			<AlertDialog.Root bind:open={examKindDialogOpen}>
				<AlertDialog.Content>
					<AlertDialog.Header>
						<AlertDialog.Title>ยืนยันการเปลี่ยนชนิดรอบสอบ</AlertDialog.Title>
						<AlertDialog.Description>
							เปลี่ยนชนิดรอบสอบเป็น{pendingExamKind
								? examRoundKindLabel(pendingExamKind)
								: 'รอบสอบใหม่'}? รายการสอบที่นำเข้าไว้ {examScheduleItemCount} รายการจะไม่ถูกลบอัตโนมัติ
						</AlertDialog.Description>
					</AlertDialog.Header>
					<AlertDialog.Footer>
						<AlertDialog.Cancel onclick={cancelExamKindChange}>ยกเลิก</AlertDialog.Cancel>
						<AlertDialog.Action onclick={confirmExamKindChange}
							>เปลี่ยนชนิดรอบสอบ</AlertDialog.Action
						>
					</AlertDialog.Footer>
				</AlertDialog.Content>
			</AlertDialog.Root>

			<AlertDialog.Root bind:open={clearMismatchedDialogOpen}>
				<AlertDialog.Content>
					<AlertDialog.Header>
						<AlertDialog.Title>ยืนยันการล้างรายการสอบ</AlertDialog.Title>
						<AlertDialog.Description>
							ล้างรายการสอบที่ไม่ใช่{examRoundKindLabel(workspace.round.examKind)}?
							รายการที่เคยจัดตารางไว้ของชุดนั้นจะถูกเอาออกด้วย
						</AlertDialog.Description>
					</AlertDialog.Header>
					<AlertDialog.Footer>
						<AlertDialog.Cancel>ยกเลิก</AlertDialog.Cancel>
						<AlertDialog.Action variant="destructive" onclick={confirmClearMismatchedItems}>
							ล้างรายการไม่ตรงรอบสอบ
						</AlertDialog.Action>
					</AlertDialog.Footer>
				</AlertDialog.Content>
			</AlertDialog.Root>
		</div>
	{/if}
</PageShell>
