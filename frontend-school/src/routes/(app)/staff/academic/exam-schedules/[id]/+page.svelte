<script lang="ts">
	import { toast } from 'svelte-sonner';
	import type { Alignment, Borders, Fill, Workbook, Worksheet } from 'exceljs';
	import type { PageProps } from './$types';
	import {
		getAcademicStructure,
		listClassrooms,
		type AcademicStructureData,
		type Classroom
	} from '$lib/api/academic';
	import {
		assignExamAssignmentInvigilator,
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
		removeExamAssignmentInvigilator,
		updateExamRound,
		upsertDayRoomAssignment,
		upsertExamDay,
		type ExamDayDetail,
		type ExamInvigilatorStaffOption,
		type ExamInvigilatorWorkspace,
		type ExamRoundKind,
		type ExamRoundStatus,
		type ExamScheduleItem,
		type ExamScheduleWorkspace,
		type ExamSession,
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
	import {
		buildExamScheduleExportWorkbook,
		examScheduleExportFileName,
		type ExamScheduleReportSheet,
		type ExamScheduleExportSheet
	} from '$lib/utils/exam-schedule-export';
	import { addMinutes } from '$lib/utils/examScheduleTime';
	import { Download, RefreshCw, Send } from 'lucide-svelte';

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
	let staffLoading = $state(false);
	let staffRequested = $state(false);
	let optionsLoading = $state(false);
	let optionsRequested = $state(false);
	let importing = $state(false);
	let clearingMismatchedItems = $state(false);
	let publishing = $state(false);
	let exportingExamSchedule = $state(false);
	let savingDay = $state(false);
	let deletingDayId = $state<string | null>(null);
	let savingAssignment = $state(false);
	let generatingAssignmentId = $state<string | null>(null);
	let placingItemIds = $state<string[]>([]);
	let unschedulingSessionIds = $state<string[]>([]);
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

	type PendingPlacementRollback = {
		restoredItem: ExamScheduleItem | null;
		previousSession: ExamSession | null;
		pendingSessionId: string;
	};

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
		staffLoading = false;
		staffRequested = false;
		invigilatorWorkspaceRequestToken += 1;
		staffOptionsRequestToken += 1;
		optionsRequested = false;
		optionsLoading = false;
		importing = false;
		clearingMismatchedItems = false;
		publishing = false;
		exportingExamSchedule = false;
		savingDay = false;
		deletingDayId = null;
		savingAssignment = false;
		generatingAssignmentId = null;
		placingItemIds = [];
		unschedulingSessionIds = [];
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

	function examScheduleItemFromSession(session: ExamSession): ExamScheduleItem {
		return {
			id: session.examScheduleItemId,
			examRoundId: session.examRoundId,
			academicSemesterId: session.academicSemesterId,
			assessmentCategoryId: session.assessmentCategoryId,
			assessmentPlanId: session.assessmentPlanId,
			classroomCourseId: session.classroomCourseId,
			classroomId: session.classroomId,
			subjectId: session.subjectId,
			gradeLevelId: session.gradeLevelId,
			durationMinutes: session.durationMinutes,
			importedAt: session.importedAt,
			assessmentCategoryName: session.assessmentCategoryName,
			subjectCode: session.subjectCode,
			subjectNameTh: session.subjectNameTh,
			subjectNameEn: session.subjectNameEn,
			subjectGroupId: session.subjectGroupId,
			subjectGroupName: session.subjectGroupName,
			subjectGroupDisplayOrder: session.subjectGroupDisplayOrder,
			subjectType: session.subjectType,
			classroomName: session.classroomName,
			gradeLevelName: session.gradeLevelName,
			gradeLevelType: session.gradeLevelType,
			gradeLevelYear: session.gradeLevelYear
		};
	}

	function addPlacingItemId(itemId: string): boolean {
		if (placingItemIds.includes(itemId)) return false;
		placingItemIds = [...placingItemIds, itemId];
		return true;
	}

	function removePlacingItemId(itemId: string) {
		placingItemIds = placingItemIds.filter((id) => id !== itemId);
	}

	function addUnschedulingSessionId(sessionId: string): boolean {
		if (unschedulingSessionIds.includes(sessionId)) return false;
		unschedulingSessionIds = [...unschedulingSessionIds, sessionId];
		return true;
	}

	function removeUnschedulingSessionId(sessionId: string) {
		unschedulingSessionIds = unschedulingSessionIds.filter((id) => id !== sessionId);
	}

	function assignmentForDayClassroom(day: ExamDayDetail, classroomId: string) {
		return day.roomAssignments.find((item) => item.classroomId === classroomId) ?? null;
	}

	function applyPendingExamSession(input: PlaceExamSessionInput): PendingPlacementRollback | null {
		if (!workspace) return null;

		const day = workspace.days.find((item) => item.id === input.examDayId);
		const previousSession =
			workspace.scheduledSessions.find(
				(session) => session.examScheduleItemId === input.examScheduleItemId
			) ?? null;
		const restoredItem =
			workspace.unscheduledItems.find((item) => item.id === input.examScheduleItemId) ?? null;
		const source = previousSession ?? restoredItem;
		if (!day || !source) return null;

		const assignment = assignmentForDayClassroom(day, source.classroomId);
		const pendingSession: ExamSession = {
			id: previousSession?.id ?? `pending-${input.examScheduleItemId}`,
			examScheduleItemId: input.examScheduleItemId,
			examRoundId: source.examRoundId,
			examDayId: day.id,
			startsAt: input.startsAt,
			endsAt: addMinutes(input.startsAt, source.durationMinutes),
			academicSemesterId: source.academicSemesterId,
			assessmentCategoryId: source.assessmentCategoryId,
			assessmentPlanId: source.assessmentPlanId,
			classroomCourseId: source.classroomCourseId,
			classroomId: source.classroomId,
			subjectId: source.subjectId,
			gradeLevelId: source.gradeLevelId,
			durationMinutes: source.durationMinutes,
			importedAt: source.importedAt,
			examDate: day.examDate,
			assessmentCategoryName: source.assessmentCategoryName,
			subjectCode: source.subjectCode,
			subjectNameTh: source.subjectNameTh,
			subjectNameEn: source.subjectNameEn,
			subjectGroupId: source.subjectGroupId,
			subjectGroupName: source.subjectGroupName,
			subjectGroupDisplayOrder: source.subjectGroupDisplayOrder,
			subjectType: source.subjectType,
			classroomName: source.classroomName,
			gradeLevelName: source.gradeLevelName,
			gradeLevelType: source.gradeLevelType,
			gradeLevelYear: source.gradeLevelYear,
			roomId: assignment?.roomId ?? previousSession?.roomId ?? null,
			roomName: assignment?.roomName ?? previousSession?.roomName ?? null,
			buildingName: previousSession?.buildingName ?? null,
			invigilators: assignment?.invigilators ?? previousSession?.invigilators ?? []
		};

		workspace = {
			...workspace,
			unscheduledItems: restoredItem
				? workspace.unscheduledItems.filter((item) => item.id !== restoredItem.id)
				: workspace.unscheduledItems,
			scheduledSessions: [
				...workspace.scheduledSessions.filter(
					(session) => session.examScheduleItemId !== input.examScheduleItemId
				),
				pendingSession
			]
		};

		return {
			restoredItem,
			previousSession,
			pendingSessionId: pendingSession.id
		};
	}

	function rollbackPendingExamSession(rollback: PendingPlacementRollback) {
		if (!workspace) return;

		const scheduledSessions = workspace.scheduledSessions.filter(
			(session) =>
				session.id !== rollback.pendingSessionId &&
				session.examScheduleItemId !== rollback.previousSession?.examScheduleItemId
		);
		if (rollback.previousSession) {
			scheduledSessions.push(rollback.previousSession);
		}

		workspace = {
			...workspace,
			unscheduledItems:
				rollback.restoredItem &&
				!workspace.unscheduledItems.some((item) => item.id === rollback.restoredItem?.id)
					? [...workspace.unscheduledItems, rollback.restoredItem]
					: workspace.unscheduledItems,
			scheduledSessions
		};
	}

	function applyPlacedExamSession(session: ExamSession) {
		if (!workspace) return;

		workspace = {
			...workspace,
			unscheduledItems: workspace.unscheduledItems.filter(
				(item) => item.id !== session.examScheduleItemId
			),
			scheduledSessions: [
				...workspace.scheduledSessions.filter(
					(item) => item.id !== session.id && item.examScheduleItemId !== session.examScheduleItemId
				),
				session
			]
		};
	}

	function applyPendingRemovedExamSession(session: ExamSession) {
		if (!workspace) return;

		workspace = {
			...workspace,
			scheduledSessions: workspace.scheduledSessions.filter((item) => item.id !== session.id)
		};
	}

	function rollbackPendingRemovedExamSession(session: ExamSession) {
		if (!workspace) return;

		workspace = {
			...workspace,
			unscheduledItems: workspace.unscheduledItems.filter(
				(item) => item.id !== session.examScheduleItemId
			),
			scheduledSessions: workspace.scheduledSessions.some((item) => item.id === session.id)
				? workspace.scheduledSessions
				: [...workspace.scheduledSessions, session]
		};
	}

	function applyRemovedExamSession(session: ExamSession) {
		if (!workspace) return;

		const restoredItem = examScheduleItemFromSession(session);
		workspace = {
			...workspace,
			unscheduledItems: workspace.unscheduledItems.some((item) => item.id === restoredItem.id)
				? workspace.unscheduledItems
				: [...workspace.unscheduledItems, restoredItem],
			scheduledSessions: workspace.scheduledSessions.filter((item) => item.id !== session.id)
		};
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
			const staffOptions = await listExamInvigilatorStaffOptions(roundId, { limit: 500 });
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

	async function ensureInvigilatorWorkspaceForExport(
		roundId: string
	): Promise<ExamInvigilatorWorkspace | null> {
		if (invigilatorWorkspace?.roundId === roundId) return invigilatorWorkspace;

		const invigilatorData = await getExamInvigilatorWorkspace(roundId);
		invigilatorWorkspace = invigilatorData;
		invigilatorLoadError = '';
		return invigilatorData;
	}

	const reportFontName = 'TH Sarabun New';
	const thinTableBorder: Partial<Borders> = {
		top: { style: 'thin' },
		left: { style: 'thin' },
		bottom: { style: 'thin' },
		right: { style: 'thin' }
	};
	const tableHeaderFill: Fill = {
		type: 'pattern',
		pattern: 'solid',
		fgColor: { argb: 'FFEFEFEF' }
	};
	const centeredAlignment: Partial<Alignment> = {
		horizontal: 'center',
		vertical: 'middle',
		wrapText: true
	};
	const leftAlignment: Partial<Alignment> = {
		horizontal: 'left',
		vertical: 'middle',
		wrapText: true
	};

	function applyWorksheetColumns(worksheet: Worksheet, exportSheet: ExamScheduleExportSheet) {
		for (const [index, column] of exportSheet['!cols']?.entries() ?? []) {
			worksheet.getColumn(index + 1).width = column.wch;
		}
	}

	function reportSheetColumnCount(reportSheet: ExamScheduleReportSheet) {
		return Math.max(
			reportSheet['!cols']?.length ?? 0,
			...reportSheet.rows.map((row) => row.length),
			1
		);
	}

	function isPaperTransferSheet(reportSheet: ExamScheduleReportSheet) {
		return reportSheet.name === 'รับส่งข้อสอบ';
	}

	function isBlankReportRow(reportSheet: ExamScheduleReportSheet, rowNumber: number) {
		const row = reportSheet.rows[rowNumber - 1] ?? [];
		return row.length === 0 || row.every((cell) => String(cell ?? '').trim() === '');
	}

	function isPaperTransferHeaderRow(reportSheet: ExamScheduleReportSheet, rowNumber: number) {
		const row = reportSheet.rows[rowNumber - 1] ?? [];
		return isPaperTransferSheet(reportSheet) && row[0] === 'วิชา' && row[1] === 'รหัสวิชา';
	}

	function isPaperTransferTimeRow(reportSheet: ExamScheduleReportSheet, rowNumber: number) {
		const row = reportSheet.rows[rowNumber - 1] ?? [];
		return isPaperTransferSheet(reportSheet) && String(row[0] ?? '').startsWith('เวลา ');
	}

	function isPaperTransferDayRow(reportSheet: ExamScheduleReportSheet, rowNumber: number) {
		const row = reportSheet.rows[rowNumber - 1] ?? [];
		return (
			isPaperTransferSheet(reportSheet) &&
			rowNumber > 3 &&
			row.length === 1 &&
			String(row[0] ?? '').trim() !== '' &&
			!isPaperTransferTimeRow(reportSheet, rowNumber)
		);
	}

	function reportHeaderText(reportSheet: ExamScheduleReportSheet, columnIndex: number) {
		if (isPaperTransferSheet(reportSheet)) {
			const headerRow = reportSheet.rows.find((row) => row[0] === 'วิชา' && row[1] === 'รหัสวิชา');
			return String(headerRow?.[columnIndex] ?? '');
		}
		return String(reportSheet.rows[3]?.[columnIndex] ?? '');
	}

	function columnTextWidth(value: string | number | undefined) {
		return String(value ?? '')
			.split(/\r?\n/)
			.reduce((width, line) => Math.max(width, line.trim().length), 0);
	}

	function reportColumnWidthBounds(headerText: string) {
		if (headerText === 'วันสอบ' || headerText === 'วันเดือนปี') return { min: 20, max: 30 };
		if (headerText === 'เวลา') return { min: 14, max: 20 };
		if (headerText === 'เวลาสอบ') return { min: 10, max: 15 };
		if (headerText === 'รหัสวิชา') return { min: 11, max: 16 };
		if (headerText === 'วิชา') return { min: 20, max: 44 };
		if (headerText === 'ชั้น' || headerText === 'ชั้น/ห้อง' || headerText === 'ห้องเรียน') {
			return { min: 10, max: 16 };
		}
		if (headerText === 'ห้องสอบ') return { min: 12, max: 22 };
		if (headerText === 'กรรมการคุมสอบ') return { min: 18, max: 34 };
		if (headerText.startsWith('ลงชื่อ')) return { min: 18, max: 24 };
		if (headerText === 'ลงชื่อรับข้อสอบ' || headerText === 'ลงชื่อส่งข้อสอบ') {
			return { min: 20, max: 28 };
		}
		if (headerText === 'เวลารับ' || headerText === 'เวลาส่ง') return { min: 9, max: 12 };
		if (headerText === 'หมายเหตุ') return { min: 12, max: 22 };
		return { min: 8, max: 28 };
	}

	function autoFitWorksheetColumns(worksheet: Worksheet, reportSheet: ExamScheduleReportSheet) {
		const columnCount = reportSheetColumnCount(reportSheet);
		for (let columnIndex = 0; columnIndex < columnCount; columnIndex += 1) {
			const headerText = reportHeaderText(reportSheet, columnIndex);
			const { min, max } = reportColumnWidthBounds(headerText);
			const widest = reportSheet.rows.reduce(
				(width, row) => Math.max(width, columnTextWidth(row[columnIndex])),
				columnTextWidth(headerText)
			);
			worksheet.getColumn(columnIndex + 1).width = Math.min(max, Math.max(min, widest + 2));
		}
	}

	function applyWorksheetMerges(worksheet: Worksheet, exportSheet: ExamScheduleExportSheet) {
		for (const merge of exportSheet['!merges'] ?? []) {
			worksheet.mergeCells(merge.s.r + 1, merge.s.c + 1, merge.e.r + 1, merge.e.c + 1);
		}
	}

	function applyWorksheetPageBreaks(worksheet: Worksheet, exportSheet: ExamScheduleExportSheet) {
		for (const rowIndex of exportSheet['!rowBreaks'] ?? []) {
			worksheet.getRow(rowIndex + 1).addPageBreak();
		}
	}

	function reportCellBorder(
		reportSheet: ExamScheduleReportSheet,
		rowNumber: number,
		columnNumber: number
	): Partial<Borders> {
		const isInvigilatorSummarySheet = reportSheet.name === 'กรรมการคุมสอบ';
		if (!isInvigilatorSummarySheet || rowNumber <= 4) return thinTableBorder;

		const border: Partial<Borders> = { ...thinTableBorder };
		if (columnNumber === 4) delete border.right;
		if (columnNumber === 5) delete border.left;
		return border;
	}

	function styleReportSheet(worksheet: Worksheet, reportSheet: ExamScheduleReportSheet) {
		const columnCount = reportSheetColumnCount(reportSheet);
		const reportIsPaperTransferSheet = isPaperTransferSheet(reportSheet);
		worksheet.pageSetup = {
			paperSize: 9,
			orientation: 'portrait',
			fitToPage: true,
			fitToWidth: 1,
			fitToHeight: 0,
			margins: {
				left: 0.25,
				right: 0.25,
				top: 0.45,
				bottom: 0.45,
				header: 0.2,
				footer: 0.2
			},
			printTitlesRow: reportSheet['!printTitlesRow'] ?? '1:4'
		};
		worksheet.views = [{ state: 'frozen', ySplit: reportIsPaperTransferSheet ? 2 : 4 }];
		worksheet.getRow(1).height = 24;
		worksheet.getRow(2).height = 22;
		worksheet.getRow(3).height = 6;

		for (let rowNumber = 1; rowNumber <= worksheet.rowCount; rowNumber += 1) {
			const row = worksheet.getRow(rowNumber);
			row.font = { name: reportFontName, size: 16 };

			if (rowNumber === 1) {
				row.font = { name: reportFontName, size: 18, bold: true };
				row.alignment = centeredAlignment;
				continue;
			}

			if (rowNumber === 2) {
				row.font = { name: reportFontName, size: 16, bold: true };
				row.alignment = centeredAlignment;
				continue;
			}

			if (isBlankReportRow(reportSheet, rowNumber)) {
				row.height = 8;
				continue;
			}

			if (rowNumber < 4) continue;
			const isPaperTransferHeader = isPaperTransferHeaderRow(reportSheet, rowNumber);
			const isPaperTransferTime = isPaperTransferTimeRow(reportSheet, rowNumber);
			const isPaperTransferDay = isPaperTransferDayRow(reportSheet, rowNumber);
			row.height =
				reportIsPaperTransferSheet && rowNumber > 4 && !isPaperTransferTime && !isPaperTransferDay
					? 30
					: 22;
			if (isPaperTransferHeader) row.height = 42;

			for (let columnNumber = 1; columnNumber <= columnCount; columnNumber += 1) {
				const cell = row.getCell(columnNumber);
				const headerText = reportHeaderText(reportSheet, columnNumber - 1);
				const isSecondInvigilatorColumn =
					reportSheet.name === 'กรรมการคุมสอบ' && columnNumber === 5;
				const isTableHeader =
					(rowNumber === 4 && !reportIsPaperTransferSheet) || isPaperTransferHeader;
				const isPaperTransferSubjectCell =
					reportIsPaperTransferSheet &&
					!isPaperTransferHeader &&
					!isPaperTransferDay &&
					!isPaperTransferTime &&
					headerText === 'วิชา';
				const shouldAlignLeft =
					isPaperTransferTime ||
					isPaperTransferSubjectCell ||
					(!reportIsPaperTransferSheet &&
						rowNumber > 4 &&
						(headerText === 'วิชา' ||
							headerText === 'กรรมการคุมสอบ' ||
							headerText === 'หมายเหตุ' ||
							isSecondInvigilatorColumn));
				cell.font = {
					name: reportFontName,
					size: 16,
					bold: isTableHeader || isPaperTransferDay || isPaperTransferTime,
					italic: isPaperTransferTime
				};
				cell.border = reportCellBorder(reportSheet, rowNumber, columnNumber);
				cell.alignment = shouldAlignLeft ? leftAlignment : centeredAlignment;
				if (isTableHeader || isPaperTransferDay || isPaperTransferTime) {
					cell.fill = tableHeaderFill;
				}
			}
		}
	}

	function styleObjectSheet(worksheet: Worksheet) {
		worksheet.views = [{ state: 'frozen', ySplit: 1 }];
		worksheet.eachRow((row, rowNumber) => {
			row.font = { name: reportFontName, size: 14, bold: rowNumber === 1 };
			row.alignment = rowNumber === 1 ? centeredAlignment : leftAlignment;
			row.height = rowNumber === 1 ? 22 : 20;
			row.eachCell({ includeEmpty: true }, (cell) => {
				cell.border = thinTableBorder;
				cell.alignment = rowNumber === 1 ? centeredAlignment : leftAlignment;
				if (rowNumber === 1) {
					cell.fill = tableHeaderFill;
				}
			});
		});
	}

	function appendReportSheet(workbook: Workbook, reportSheet: ExamScheduleReportSheet) {
		const worksheet = workbook.addWorksheet(reportSheet.name);
		worksheet.addRows(reportSheet.rows);
		applyWorksheetColumns(worksheet, reportSheet);
		autoFitWorksheetColumns(worksheet, reportSheet);
		applyWorksheetMerges(worksheet, reportSheet);
		applyWorksheetPageBreaks(worksheet, reportSheet);
		styleReportSheet(worksheet, reportSheet);
	}

	function appendObjectSheet(
		workbook: Workbook,
		name: string,
		exportSheet: ExamScheduleExportSheet<Record<string, string | number>>
	) {
		const worksheet = workbook.addWorksheet(name);
		const headers = Object.keys(exportSheet.rows[0] ?? {});
		worksheet.columns = headers.map((header, index) => ({
			header,
			key: header,
			width: exportSheet['!cols']?.[index]?.wch ?? 16
		}));

		for (const row of exportSheet.rows) {
			worksheet.addRow(row);
		}
		styleObjectSheet(worksheet);
	}

	function saveWorkbookBuffer(buffer: ArrayBuffer, fileName: string) {
		const blob = new Blob([buffer], {
			type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'
		});
		const url = URL.createObjectURL(blob);
		const link = document.createElement('a');
		link.href = url;
		link.download = fileName;
		document.body.appendChild(link);
		link.click();
		link.remove();
		URL.revokeObjectURL(url);
	}

	async function handleExportExamSchedule() {
		if (!workspace || exportingExamSchedule) return;

		exportingExamSchedule = true;
		try {
			const invigilatorData = await ensureInvigilatorWorkspaceForExport(workspace.round.id);
			const ExcelJSModule = await import('exceljs');
			const ExcelJS = ExcelJSModule.default;
			const exportWorkbook = buildExamScheduleExportWorkbook(workspace, invigilatorData, {
				classrooms
			});
			const workbook = new ExcelJS.Workbook();
			workbook.creator = 'SchoolOrbit';
			workbook.created = new Date();
			workbook.modified = new Date();

			for (const reportSheet of exportWorkbook.reportSheets) {
				appendReportSheet(workbook, reportSheet);
			}

			appendObjectSheet(workbook, 'ห้องสอบ', exportWorkbook.rooms);
			appendObjectSheet(workbook, 'ภาระงานกรรมการ', exportWorkbook.workloads);
			appendObjectSheet(workbook, 'ความพร้อม', exportWorkbook.readiness);

			const buffer = await workbook.xlsx.writeBuffer();
			saveWorkbookBuffer(buffer, examScheduleExportFileName(workspace.round.name));
			toast.success('ส่งออกตารางสอบแล้ว');
		} catch (exportError) {
			toast.error(exportError instanceof Error ? exportError.message : 'ส่งออกตารางสอบไม่สำเร็จ');
		} finally {
			exportingExamSchedule = false;
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

	async function handleAssignInvigilator(
		assignmentId: string,
		staffId: string
	): Promise<ExamInvigilatorWorkspace> {
		try {
			const updatedWorkspace = await assignExamAssignmentInvigilator(assignmentId, staffId);
			invigilatorWorkspace = updatedWorkspace;
			toast.success('บันทึกกรรมการคุมสอบแล้ว');
			return updatedWorkspace;
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'บันทึกกรรมการคุมสอบไม่สำเร็จ');
			throw saveError;
		}
	}

	async function handleRemoveInvigilator(
		assignmentId: string,
		staffId: string
	): Promise<ExamInvigilatorWorkspace> {
		try {
			const updatedWorkspace = await removeExamAssignmentInvigilator(assignmentId, staffId);
			invigilatorWorkspace = updatedWorkspace;
			toast.success('ลบกรรมการคุมสอบแล้ว');
			return updatedWorkspace;
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'ลบกรรมการคุมสอบไม่สำเร็จ');
			throw saveError;
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
		if (!addPlacingItemId(input.examScheduleItemId)) return false;

		const rollback = applyPendingExamSession(input);
		if (!rollback) {
			removePlacingItemId(input.examScheduleItemId);
			toast.error('ไม่พบรายการสอบสำหรับจัดเวลา');
			return false;
		}

		try {
			const session = await placeExamSession({
				examScheduleItemId: input.examScheduleItemId,
				examDayId: input.examDayId,
				startsAt: input.startsAt
			});
			applyPlacedExamSession(session);
			void refreshOrInvalidateInvigilators(session.examRoundId);
			toast.success('บันทึกเวลาสอบแล้ว');
			return true;
		} catch (placeError) {
			rollbackPendingExamSession(rollback);
			toast.error(placeError instanceof Error ? placeError.message : 'บันทึกเวลาสอบไม่สำเร็จ');
			return false;
		} finally {
			removePlacingItemId(input.examScheduleItemId);
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
		if (!addUnschedulingSessionId(sessionId)) return false;

		applyPendingRemovedExamSession(session);
		try {
			await deleteExamSession(sessionId);
			applyRemovedExamSession(session);
			void refreshOrInvalidateInvigilators(session.examRoundId);
			toast.success('เอารายการสอบออกจากตารางแล้ว');
			return true;
		} catch (deleteError) {
			rollbackPendingRemovedExamSession(session);
			toast.error(
				deleteError instanceof Error ? deleteError.message : 'เอารายการสอบออกจากตารางไม่สำเร็จ'
			);
			return false;
		} finally {
			removeUnschedulingSessionId(sessionId);
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
	class="flex h-full min-h-0 flex-col"
	contentClass="flex min-h-0 flex-1 flex-col"
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
				<LoadingButton
					variant="outline"
					size="sm"
					loading={exportingExamSchedule}
					loadingLabel="กำลังส่งออก..."
					onclick={handleExportExamSchedule}
				>
					<Download class="h-4 w-4" />
					ส่งออก
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
		<div class="flex min-h-0 flex-1 flex-col">
			<Tabs.Root bind:value={activeTab} class="flex min-h-0 flex-1 flex-col gap-4">
				<Tabs.List class="grid w-full grid-cols-4 md:w-fit">
					<Tabs.Trigger value="setup">ตั้งค่า</Tabs.Trigger>
					<Tabs.Trigger value="rooms">ห้องสอบ</Tabs.Trigger>
					<Tabs.Trigger value="schedule">จัดตาราง</Tabs.Trigger>
					<Tabs.Trigger value="invigilators">กรรมการ</Tabs.Trigger>
				</Tabs.List>

				<Tabs.Content value="setup" class="min-h-0 flex-1">
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

				<Tabs.Content value="rooms" class="min-h-0 flex-1">
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

				<Tabs.Content value="schedule" class="min-h-0 flex-1">
					<ExamScheduleTimeline
						{workspace}
						readonly={!canManageExamSchedules || workspace.round.status === 'published'}
						{placingItemIds}
						{unschedulingSessionIds}
						canManageActions={canManageExamSchedules && workspace.round.status !== 'published'}
						{importing}
						{clearingMismatchedItems}
						examKindLabel={examRoundKindLabel(workspace.round.examKind)}
						onPlaceSession={handlePlaceExamSession}
						onUnscheduleSession={handleUnscheduleExamSession}
						onImportItems={handleImportItems}
						onClearMismatchedItems={handleClearMismatchedItems}
					/>
				</Tabs.Content>

				<Tabs.Content value="invigilators" class="min-h-0 flex-1">
					<ExamInvigilatorPanel
						days={workspace.days}
						workspace={invigilatorWorkspace}
						{staff}
						loading={loadingInvigilators}
						loadError={invigilatorLoadError}
						readonly={!canManageExamSchedules || workspace.round.status === 'published'}
						onAssignInvigilator={handleAssignInvigilator}
						onRemoveInvigilator={handleRemoveInvigilator}
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
