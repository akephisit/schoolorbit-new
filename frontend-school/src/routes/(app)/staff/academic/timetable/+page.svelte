<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import type { Cell, Row, Workbook, Worksheet } from 'exceljs';
	import { toast } from 'svelte-sonner';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import {
		listBellSchedulePeriods,
		listBellSchedules,
		listHomerooms,
		type BellSchedule,
		type BellSchedulePeriod,
		type Homeroom
	} from '$lib/api/academic-core';
	import {
		getAcademicTermChangeSet,
		listLearningGroupsForTerm,
		listLearningOfferings,
		type AcademicTermChangeSet,
		type LearningGroup,
		type LearningOffering
	} from '$lib/api/learning-delivery';
	import { lookupRooms, type RoomLookupItem } from '$lib/api/lookup';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import {
		createTimetableEntry,
		currentLocalDate,
		deleteTimetableEntry,
		listTimetableEntries,
		listTimetableVersions,
		updateTimetableEntry,
		type TimetableEntry,
		type TimetableVersion
	} from '$lib/api/timetable';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import {
		AcademicPrerequisiteNotice,
		type AcademicPrerequisite
	} from '$lib/components/academic-workflow';
	import AcademicChangeReadiness from '$lib/components/learning-delivery/AcademicChangeReadiness.svelte';
	import AcademicChangeSetDialog from '$lib/components/learning-delivery/AcademicChangeSetDialog.svelte';
	import TimetableInstructorPicker from '$lib/components/academic/timetable/TimetableInstructorPicker.svelte';
	import MobileDragDropPolyfill from '$lib/components/MobileDragDropPolyfill.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import {
		connectTimetableSocket,
		disconnectTimetableSocket,
		refreshTrigger
	} from '$lib/stores/timetable-socket';
	import { loadTimetableCollections } from '$lib/workspaces/academic-batch';
	import {
		buildTeacherLoadExportRows,
		calculateTeacherLoadColumnWidths,
		TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS,
		TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS,
		type TeacherLoadExportRows
	} from '$lib/utils/timetable-teacher-load-export';
	import {
		AlertTriangle,
		CalendarClock,
		FileSpreadsheet,
		History,
		Loader2,
		Plus,
		RotateCcw,
		Save,
		Trash2
	} from 'lucide-svelte';

	type ViewKind = 'learning_group' | 'homeroom';
	type EntryType = 'COURSE' | 'ACTIVITY' | 'HOMEROOM' | 'ACADEMIC' | 'BREAK';

	const dayOptions = [
		{ value: 'MON', label: 'จันทร์' },
		{ value: 'TUE', label: 'อังคาร' },
		{ value: 'WED', label: 'พุธ' },
		{ value: 'THU', label: 'พฤหัสบดี' },
		{ value: 'FRI', label: 'ศุกร์' },
		{ value: 'SAT', label: 'เสาร์' },
		{ value: 'SUN', label: 'อาทิตย์' }
	];
	const entryTypeOptions: Array<{ value: EntryType; label: string }> = [
		{ value: 'COURSE', label: 'รายวิชา' },
		{ value: 'ACTIVITY', label: 'กิจกรรม' },
		{ value: 'HOMEROOM', label: 'โฮมรูม' },
		{ value: 'ACADEMIC', label: 'กิจกรรมวิชาการ' },
		{ value: 'BREAK', label: 'พัก' }
	];
	const NO_ROOM_VALUE = '__no_room__';
	const missingGroupsPrerequisite: AcademicPrerequisite = {
		key: 'timetable-learning-groups',
		status: 'missing',
		title: 'ยังไม่มีกลุ่มเรียนสำหรับจัดตาราง',
		description: 'สร้างกลุ่มใต้รายการเปิดสอน และกำหนดผู้เรียนหรือห้องต้นทางให้ตรงกับการสอนจริง',
		actionLabel: 'ไปจัดกลุ่มเรียน',
		href: '/staff/academic/delivery'
	};
	const missingTeachersPrerequisite: AcademicPrerequisite = {
		key: 'timetable-teachers',
		status: 'warning',
		title: 'บางกลุ่มยังไม่มีครูผู้สอน',
		description: 'กำหนดครูให้กลุ่มเรียนก่อนวางคาบ เพื่อให้ตรวจตารางชนและสรุปภาระงานได้ถูกต้อง',
		actionLabel: 'ไปกำหนดครูในกลุ่มเรียน',
		href: '/staff/academic/delivery'
	};
	const missingPeriodsPrerequisite: AcademicPrerequisite = {
		key: 'timetable-periods',
		status: 'missing',
		title: 'ยังไม่มีตารางเวลาและคาบเรียน',
		description: 'ตั้งเวลาเริ่มและสิ้นสุดของแต่ละคาบในปีการศึกษา ก่อนนำมาใช้จัดตารางสอน',
		actionLabel: 'ไปตั้งค่าคาบเรียน',
		href: '/staff/academic/core#bell-schedules'
	};
	const missingRoomsPrerequisite: AcademicPrerequisite = {
		key: 'timetable-rooms',
		status: 'warning',
		title: 'ยังไม่มีห้องเรียนให้เลือก',
		description: 'เพิ่มอาคารและห้องเรียนในข้อมูลงานอาคาร แล้วกลับมาเลือกห้องให้แต่ละคาบ',
		actionLabel: 'ไปจัดการอาคารและห้อง',
		href: '/staff/facility/buildings'
	};

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const academicYearId = $derived($academicContext.selected.academicYearId);
	let schedules = $state<BellSchedule[]>([]);
	let periods = $state<BellSchedulePeriod[]>([]);
	let offerings = $state<LearningOffering[]>([]);
	let groups = $state<LearningGroup[]>([]);
	let homerooms = $state<Homeroom[]>([]);
	let rooms = $state<RoomLookupItem[]>([]);
	let entries = $state<TimetableEntry[]>([]);
	let versions = $state<TimetableVersion[]>([]);
	let selectedVersion = $state.raw<TimetableVersion | null>(null);
	let selectedChangeSet = $state.raw<AcademicTermChangeSet | null>(null);
	let versionSelectValue = $state('');
	let selectedScheduleId = $state('');
	let viewKind = $state<ViewKind>('learning_group');
	let selectedTargetId = $state('');
	let targetSelectValue = $state('');
	let selectedEntryId = $state('');
	let formDay = $state('MON');
	let formPeriodId = $state('');
	let formRoomId = $state('');
	let formEntryType = $state<EntryType>('COURSE');
	let formInstructorIds = $state<string[]>([]);
	let formTitle = $state('');
	let formNote = $state('');
	let loading = $state(false);
	let busy = $state(false);
	let draftRevision = $state(0);
	let isTeacherLoadExporting = $state(false);
	let dirty = $state(false);
	let errorMessage = $state('');
	const request = new LatestRequest();

	const canRead = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_READ_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_READ_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_READ_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_READ_ASSIGNED,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	const canManage = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	const canEditSelected = $derived(canManage && selectedVersion?.status === 'draft');
	const activeDraftVersion = $derived(
		versions.find((version) => version.status === 'draft' && version.changeSetId) ?? null
	);
	const selectedEntry = $derived(entries.find((entry) => entry.id === selectedEntryId) ?? null);
	const selectedSchedule = $derived(
		schedules.find((schedule) => schedule.id === selectedScheduleId) ?? null
	);
	const groupsWithoutTeachers = $derived(
		groups.filter((group) => group.teacherAssignments.length === 0).length
	);
	const selectedGroupInstructorOptions = $derived.by(() => {
		if (viewKind !== 'learning_group' || !selectedVersion) return [];
		const group = groups.find((item) => item.id === selectedTargetId);
		if (!group) return [];
		const effectiveOn = selectedVersion.effectiveFrom;
		return group.teacherAssignments
			.filter(
				(assignment) =>
					assignment.startsOn <= effectiveOn &&
					(!assignment.endsOn || assignment.endsOn >= effectiveOn)
			)
			.filter(
				(assignment, index, assignments) =>
					assignments.findIndex((item) => item.teacherId === assignment.teacherId) === index
			)
			.map((assignment) => ({
				id: assignment.teacherId,
				displayName: assignment.displayName,
				role: assignment.role
			}));
	});
	const showsInstructorPicker = $derived(
		viewKind === 'learning_group' && (formEntryType === 'COURSE' || formEntryType === 'ACTIVITY')
	);
	const unavailableSelectedInstructors = $derived.by(() => {
		if (!selectedEntry || !showsInstructorPicker) return [];
		return selectedEntry.instructors.filter(
			(instructor) =>
				formInstructorIds.includes(instructor.userId) &&
				!selectedGroupInstructorOptions.some((option) => option.id === instructor.userId)
		);
	});
	const activeDays = $derived.by(() => {
		const selectedYear = $academicContext.options?.years.find((year) => year.id === academicYearId);
		void selectedYear;
		const configured = new Set(
			periods.flatMap((period) =>
				(period.applicableDays ?? '')
					.split(',')
					.map((day) => day.trim())
					.filter(Boolean)
			)
		);
		return configured.size > 0
			? dayOptions.filter((day) => configured.has(day.value))
			: dayOptions.slice(0, 5);
	});
	const visibleEntries = $derived(
		entries.filter((entry) => {
			if (entry.bellScheduleId !== selectedScheduleId) return false;
			return viewKind === 'learning_group'
				? entry.learningGroupId === selectedTargetId
				: entry.homeroomId === selectedTargetId;
		})
	);

	function offeringForGroup(group: LearningGroup): LearningOffering | undefined {
		return offerings.find((offering) => offering.id === group.learningOfferingId);
	}

	function groupLabel(group: LearningGroup): string {
		const offering = offeringForGroup(group);
		return `${offering?.codeSnapshot ?? ''} · ${group.code} ${group.name}`.trim();
	}

	function entryTitle(entry: TimetableEntry): string {
		return (
			entry.offeringCode ??
			entry.title ??
			entry.activityVersionDisplayLabel ??
			entry.subjectVersionDisplayLabel ??
			entry.entryType
		);
	}

	function instructorRoleLabel(role: string): string {
		if (role === 'primary') return 'ครูหลัก';
		if (role === 'secondary') return 'ครูร่วมสอน';
		if (role === 'assistant') return 'ครูผู้ช่วย';
		return 'ครูผู้สอน';
	}

	function defaultInstructorIds(): string[] {
		const teacherAssignments = selectedGroupInstructorOptions;
		return teacherAssignments.length === 1 ? [teacherAssignments[0].id] : [];
	}

	function entriesForCell(day: string, periodId: string): TimetableEntry[] {
		return visibleEntries.filter(
			(entry) => entry.dayOfWeek === day && entry.bellSchedulePeriodId === periodId
		);
	}

	function selectPreferredVersion(loadedVersions: TimetableVersion[]): TimetableVersion | null {
		const requestedId = page.url.searchParams.get('timetableVersionId');
		const explicit = loadedVersions.find((version) => version.id === requestedId);
		if (explicit) return explicit;

		const today = currentLocalDate();
		const current = loadedVersions.find(
			(version) =>
				version.status === 'published' &&
				(version.displayState === 'current' ||
					(version.effectiveFrom <= today &&
						(!version.effectiveUntil || version.effectiveUntil >= today)))
		);
		if (current) return current;

		const upcoming = loadedVersions
			.filter(
				(version) =>
					version.status === 'published' &&
					(version.displayState === 'upcoming' || version.effectiveFrom > today)
			)
			.toSorted((left, right) => left.effectiveFrom.localeCompare(right.effectiveFrom))[0];
		if (upcoming) return upcoming;

		return loadedVersions.find((version) => version.status === 'draft') ?? null;
	}

	function versionStatusLabel(version: TimetableVersion): string {
		if (version.status === 'draft') return 'แบบร่าง';
		if (version.status === 'cancelled') return 'ยกเลิกแล้ว';
		if (version.displayState === 'current') return 'เผยแพร่แล้ว · กำลังใช้';
		if (version.displayState === 'upcoming') return 'เผยแพร่แล้ว · รอเริ่มใช้';
		return 'เผยแพร่แล้ว · ประวัติ';
	}

	function versionPeriodLabel(version: TimetableVersion): string {
		return `${version.effectiveFrom} – ${version.effectiveUntil ?? 'ต่อเนื่อง'}`;
	}

	function syncVersionUrl(versionId: string): void {
		if (page.url.searchParams.get('timetableVersionId') === versionId) return;
		const nextUrl = new URL(page.url);
		nextUrl.searchParams.set('timetableVersionId', versionId);
		window.history.replaceState(window.history.state, '', nextUrl);
	}

	async function loadPeriods(
		scheduleId: string,
		signal?: AbortSignal
	): Promise<BellSchedulePeriod[]> {
		return (await listBellSchedulePeriods(scheduleId, { signal })).sort(
			(a, b) => a.orderIndex - b.orderIndex
		);
	}

	async function loadVersionOptions(
		termId: string,
		signal?: AbortSignal
	): Promise<TimetableVersion[]> {
		return listTimetableVersions(termId, { signal });
	}

	async function loadSelectedChangeSet(
		version: TimetableVersion,
		signal?: AbortSignal
	): Promise<AcademicTermChangeSet | null> {
		return version.changeSetId
			? getAcademicTermChangeSet(version.changeSetId, { signal })
			: Promise.resolve(null);
	}

	async function loadWorkspace(termId: string, yearId: string): Promise<void> {
		const { revision, signal } = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const [collections, loadedSchedules, loadedRooms, loadedVersions] = await Promise.all([
				loadTimetableCollections(
					{ listLearningOfferings, listLearningGroupsForTerm, listHomerooms },
					termId,
					yearId,
					signal
				),
				listBellSchedules(yearId, { signal }),
				lookupRooms({ activeOnly: true, limit: 500 }, { signal }),
				loadVersionOptions(termId, signal)
			]);
			const preferredVersion = selectPreferredVersion(loadedVersions);
			const preferredSchedule = loadedSchedules.find(
				(schedule) => schedule.id === preferredVersion?.bellScheduleId
			);
			const loadedPeriods = preferredSchedule
				? await loadPeriods(preferredSchedule.id, signal)
				: [];
			const loadedEntries = preferredVersion
				? await listTimetableEntries(
						{ academicTermId: termId, timetableVersionId: preferredVersion.id },
						{ signal }
					)
				: [];
			const loadedChangeSet = preferredVersion
				? await loadSelectedChangeSet(preferredVersion, signal)
				: null;
			if (!request.isCurrent(revision)) return;
			schedules = loadedSchedules;
			versions = loadedVersions;
			selectedVersion = preferredVersion;
			versionSelectValue = preferredVersion?.id ?? '';
			selectedScheduleId = preferredSchedule?.id ?? '';
			periods = loadedPeriods;
			offerings = collections.offerings;
			groups = collections.groups;
			homerooms = collections.homerooms;
			rooms = loadedRooms;
			entries = loadedEntries;
			selectedChangeSet = loadedChangeSet;
			draftRevision += 1;
			if (preferredVersion) syncVersionUrl(preferredVersion.id);
			viewKind = groups.length > 0 ? 'learning_group' : 'homeroom';
			selectedTargetId = groups[0]?.id ?? homerooms[0]?.id ?? '';
			targetSelectValue = selectedTargetId;
			resetForm();
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดพื้นที่จัดตารางสอนไม่สำเร็จ';
			}
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function refreshEntries(): Promise<void> {
		if (!academicTermId || !selectedVersion) return;
		const { revision, signal } = request.begin();
		try {
			const loadedEntries = await listTimetableEntries(
				{ academicTermId, timetableVersionId: selectedVersion.id },
				{ signal }
			);
			if (request.isCurrent(revision)) entries = loadedEntries;
		} catch (error) {
			if (!isAbortError(error)) throw error;
		}
	}

	async function changeVersion(nextId: string): Promise<void> {
		if (dirty) {
			versionSelectValue = selectedVersion?.id ?? '';
			toast.warning('กรุณาบันทึกหรือยกเลิกแบบร่างก่อนเปลี่ยนรุ่นตารางสอน');
			return;
		}
		const nextVersion = versions.find((version) => version.id === nextId);
		if (!nextVersion || !academicTermId) return;
		const { revision, signal } = request.begin();
		loading = true;
		try {
			const loadedPeriods = await loadPeriods(nextVersion.bellScheduleId, signal);
			const loadedEntries = await listTimetableEntries(
				{ academicTermId, timetableVersionId: nextVersion.id },
				{ signal }
			);
			const loadedChangeSet = await loadSelectedChangeSet(nextVersion, signal);
			if (!request.isCurrent(revision)) return;
			selectedVersion = nextVersion;
			selectedChangeSet = loadedChangeSet;
			versionSelectValue = nextVersion.id;
			selectedScheduleId = nextVersion.bellScheduleId;
			periods = loadedPeriods;
			entries = loadedEntries;
			syncVersionUrl(nextVersion.id);
			resetForm();
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision))
				errorMessage = error instanceof Error ? error.message : 'โหลดคาบเรียนไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) {
				versionSelectValue = selectedVersion?.id ?? '';
				loading = false;
			}
		}
	}

	async function handleRevisionCreated(created: AcademicTermChangeSet): Promise<void> {
		if (!academicTermId) return;
		selectedChangeSet = created;
		versions = await loadVersionOptions(academicTermId);
		if (!versions.some((version) => version.id === created.targetTimetableVersionId)) {
			throw new Error('สร้างแบบร่างแล้ว แต่ไม่พบรุ่นตารางสอนใหม่ กรุณาโหลดหน้าอีกครั้ง');
		}
		await changeVersion(created.targetTimetableVersionId);
		toast.success('สร้างรุ่นตารางสอนแบบร่างแล้ว');
	}

	async function handleChangeSetChanged(updated: AcademicTermChangeSet): Promise<void> {
		selectedChangeSet = updated;
		if (
			!academicTermId ||
			selectedVersion?.id !== updated.targetTimetableVersionId ||
			selectedVersion.status === updated.status
		)
			return;
		versions = await loadVersionOptions(academicTermId);
		await changeVersion(updated.targetTimetableVersionId);
		if (updated.status === 'published') toast.success('เผยแพร่รุ่นตารางสอนใหม่แล้ว');
		if (updated.status === 'cancelled') toast.success('ยกเลิกรุ่นตารางสอนแบบร่างแล้ว');
	}

	function changeViewKind(nextKind: ViewKind): void {
		if (dirty) {
			toast.warning('กรุณาบันทึกหรือยกเลิกแบบร่างก่อนเปลี่ยนมุมมอง');
			return;
		}
		viewKind = nextKind;
		selectedTargetId =
			nextKind === 'learning_group' ? (groups[0]?.id ?? '') : (homerooms[0]?.id ?? '');
		targetSelectValue = selectedTargetId;
		resetForm();
	}

	function changeTarget(nextId: string): void {
		if (dirty) {
			targetSelectValue = selectedTargetId;
			toast.warning('กรุณาบันทึกหรือยกเลิกแบบร่างก่อนเปลี่ยนกลุ่ม');
			return;
		}
		selectedTargetId = nextId;
		targetSelectValue = nextId;
		resetForm();
	}

	function resetForm(): void {
		selectedEntryId = '';
		formDay = activeDays[0]?.value ?? 'MON';
		formPeriodId = periods[0]?.id ?? '';
		formRoomId = '';
		const selectedGroup = groups.find((group) => group.id === selectedTargetId);
		formEntryType =
			viewKind === 'learning_group'
				? selectedGroup && offeringForGroup(selectedGroup)?.kind === 'activity'
					? 'ACTIVITY'
					: 'COURSE'
				: 'HOMEROOM';
		formInstructorIds = defaultInstructorIds();
		formTitle = '';
		formNote = '';
		dirty = false;
	}

	function startAtCell(day: string, periodId: string): void {
		if (!canEditSelected) return;
		resetForm();
		formDay = day;
		formPeriodId = periodId;
		dirty = true;
	}

	function editEntry(entry: TimetableEntry): void {
		selectedEntryId = entry.id;
		formDay = entry.dayOfWeek;
		formPeriodId = entry.bellSchedulePeriodId;
		formRoomId = entry.roomId ?? '';
		formEntryType = entry.entryType as EntryType;
		formInstructorIds = entry.instructors.map((item) => item.userId);
		formTitle = entry.title ?? '';
		formNote = entry.note ?? '';
		dirty = false;
	}

	function markDirty(): void {
		dirty = true;
	}

	function changeEntryType(value: string): void {
		formEntryType = value as EntryType;
		formInstructorIds = showsInstructorPicker ? defaultInstructorIds() : [];
		markDirty();
	}

	function removeUnavailableInstructor(instructorId: string): void {
		formInstructorIds = formInstructorIds.filter((id) => id !== instructorId);
		markDirty();
	}

	async function saveEntry(): Promise<void> {
		if (
			!academicTermId ||
			!selectedVersion ||
			!canEditSelected ||
			!selectedTargetId ||
			!formPeriodId ||
			unavailableSelectedInstructors.length > 0
		)
			return;
		busy = true;
		errorMessage = '';
		try {
			if (selectedEntry) {
				await updateTimetableEntry(selectedEntry.id, {
					timetableVersionId: selectedVersion.id,
					rowVersion: selectedEntry.rowVersion,
					dayOfWeek: formDay,
					bellSchedulePeriodId: formPeriodId,
					roomId: formRoomId || null,
					clearRoom: !formRoomId,
					note: formNote.trim() || null,
					clearNote: !formNote.trim(),
					title: formTitle.trim() || null,
					instructorIds: formInstructorIds
				});
			} else {
				await createTimetableEntry({
					academicTermId,
					timetableVersionId: selectedVersion.id,
					learningGroupId: viewKind === 'learning_group' ? selectedTargetId : null,
					homeroomId: viewKind === 'homeroom' ? selectedTargetId : null,
					dayOfWeek: formDay,
					bellSchedulePeriodId: formPeriodId,
					roomId: formRoomId || null,
					note: formNote.trim() || null,
					entryType: formEntryType,
					title: formTitle.trim() || null,
					instructorIds: formInstructorIds
				});
			}
			await refreshEntries();
			draftRevision += 1;
			resetForm();
			toast.success('บันทึกคาบในตารางแล้ว');
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกคาบไม่สำเร็จ';
			toast.error(errorMessage);
		} finally {
			busy = false;
		}
	}

	async function removeEntry(): Promise<void> {
		if (!selectedEntry || !selectedVersion || !canEditSelected) return;
		busy = true;
		try {
			await deleteTimetableEntry(selectedEntry.id, selectedEntry.rowVersion, selectedVersion.id);
			await refreshEntries();
			draftRevision += 1;
			resetForm();
			toast.success('ลบคาบออกจากตารางแล้ว');
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'ลบคาบไม่สำเร็จ';
			toast.error(errorMessage);
		} finally {
			busy = false;
		}
	}

	const teacherLoadFontName = 'TH Sarabun New';

	function styleTeacherLoadCell(cell: Cell, emphasized = false): void {
		cell.font = { name: teacherLoadFontName, size: 16, bold: emphasized };
		cell.alignment = { vertical: 'middle', wrapText: true };
		cell.border = {
			top: { style: 'thin', color: { argb: 'FFE2E8F0' } },
			left: { style: 'thin', color: { argb: 'FFE2E8F0' } },
			bottom: { style: 'thin', color: { argb: 'FFE2E8F0' } },
			right: { style: 'thin', color: { argb: 'FFE2E8F0' } }
		};
	}

	function styleTeacherLoadRow(row: Row, kind: 'header' | 'group' | 'detail'): void {
		row.height = kind === 'header' ? 28 : 24;
		row.eachCell({ includeEmpty: true }, (cell) => {
			styleTeacherLoadCell(cell, kind !== 'detail');
			if (kind !== 'detail') {
				cell.fill = {
					type: 'pattern',
					pattern: 'solid',
					fgColor: { argb: kind === 'header' ? 'FFE2E8F0' : 'FFF1F5F9' }
				};
			}
		});
	}

	function appendTeacherLoadSheet(
		workbook: Workbook,
		name: string,
		rows: Array<Array<string | number>>,
		widthOptions:
			| typeof TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS
			| typeof TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS
	): Worksheet {
		const worksheet = workbook.addWorksheet(name);
		worksheet.columns = calculateTeacherLoadColumnWidths(rows, widthOptions).map((width) => ({
			width
		}));
		worksheet.properties.defaultRowHeight = 24;

		for (const [index, values] of rows.entries()) {
			const row = worksheet.addRow(values);
			const firstCell = String(values[0] ?? '');
			styleTeacherLoadRow(
				row,
				index === 0 ? 'header' : firstCell.startsWith('กลุ่มสาระ:') ? 'group' : 'detail'
			);
		}

		worksheet.views = [{ state: 'frozen', ySplit: 1 }];
		worksheet.pageSetup = {
			orientation: 'landscape',
			fitToPage: true,
			fitToWidth: 1,
			fitToHeight: 0
		};
		return worksheet;
	}

	function appendTeacherLoadSummarySheet(
		workbook: Workbook,
		exportRows: TeacherLoadExportRows
	): void {
		appendTeacherLoadSheet(
			workbook,
			'สรุปต่อครู',
			exportRows.summarySheetRows,
			TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS
		);
	}

	function appendTeacherLoadDetailSheet(
		workbook: Workbook,
		exportRows: TeacherLoadExportRows
	): void {
		appendTeacherLoadSheet(
			workbook,
			'รายละเอียด',
			exportRows.detailSheetRows,
			TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS
		);
	}

	function safeFileName(value: string): string {
		return (
			value
				.replace(/[\\/:*?"<>|]/g, '-')
				.replace(/\s+/g, ' ')
				.trim() || 'สรุปคาบสอนครู'
		);
	}

	function saveTeacherLoadWorkbookBuffer(buffer: ArrayBuffer, fileName: string): void {
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

	async function handleExportTeacherLoadXlsx(): Promise<void> {
		if (!academicTermId || isTeacherLoadExporting) return;
		const exportRows = buildTeacherLoadExportRows(entries);
		if (exportRows.summaryRows.length === 0) {
			toast.error('ไม่พบคาบสอนสำหรับภาคเรียนนี้');
			return;
		}

		isTeacherLoadExporting = true;
		try {
			const ExcelJSModule = await import('exceljs');
			const ExcelJS = ExcelJSModule.default;
			const workbook = new ExcelJS.Workbook();
			workbook.creator = 'SchoolOrbit';
			workbook.created = new Date();
			workbook.modified = new Date();
			appendTeacherLoadSummarySheet(workbook, exportRows);
			appendTeacherLoadDetailSheet(workbook, exportRows);

			const selectedTerm = $academicContext.options?.terms.find(
				(term) => term.id === academicTermId
			);
			const selectedYear = $academicContext.options?.years.find(
				(year) => year.id === academicYearId
			);
			const fileName = safeFileName(
				`สรุปคาบสอนครู-${selectedTerm?.name ?? 'ภาคเรียน'}-${selectedYear?.name ?? 'ปีการศึกษา'}`
			);
			const buffer = await workbook.xlsx.writeBuffer();
			saveTeacherLoadWorkbookBuffer(buffer, `${fileName}.xlsx`);
			toast.success(`ดาวน์โหลดสรุปคาบสอน ${exportRows.summaryRows.length} คนแล้ว`);
		} catch (error) {
			console.error('Failed to export teacher load workbook', error);
			toast.error('ส่งออกสรุปคาบสอนไม่สำเร็จ');
		} finally {
			isTeacherLoadExporting = false;
		}
	}

	onMount(() => {
		let selectedTermId = '';
		let selectedYearId = '';
		let currentUserId = '';
		let loadedContextKey = '';
		let connectedSocketKey = '';
		let observedRefresh: number | null = null;

		function synchronizeContext() {
			if (!selectedTermId || !selectedYearId) return;
			const contextKey = `${selectedYearId}:${selectedTermId}`;
			if (contextKey !== loadedContextKey) {
				loadedContextKey = contextKey;
				void loadWorkspace(selectedTermId, selectedYearId);
			}
			if (!currentUserId) return;
			const socketKey = `${selectedTermId}:${currentUserId}`;
			if (socketKey !== connectedSocketKey) {
				connectedSocketKey = socketKey;
				connectTimetableSocket({ academicTermId: selectedTermId, currentUserId });
			}
		}

		const unregisterDirty = registerAcademicContextDirtySource('academic-timetable', () => dirty);
		const unsubscribeContext = academicContext.subscribe((state) => {
			selectedTermId = state.selected.academicTermId ?? '';
			selectedYearId = state.selected.academicYearId ?? '';
			synchronizeContext();
		});
		const unsubscribeAuth = authStore.subscribe((state) => {
			currentUserId = state.user?.id ?? '';
			synchronizeContext();
		});
		const unsubscribeRefresh = refreshTrigger.subscribe((value) => {
			if (observedRefresh === null) {
				observedRefresh = value;
				return;
			}
			if (value === observedRefresh) return;
			observedRefresh = value;
			if (!selectedTermId || !selectedYearId) return;
			if (dirty) {
				toast.warning(
					'มีการเปลี่ยนแปลงตารางจากผู้ใช้อื่น กรุณาบันทึกหรือยกเลิกแบบร่างก่อนโหลดใหม่'
				);
				return;
			}
			void loadWorkspace(selectedTermId, selectedYearId);
		});
		return () => {
			request.abort();
			unsubscribeRefresh();
			unsubscribeAuth();
			unsubscribeContext();
			unregisterDirty();
			disconnectTimetableSocket();
		};
	});
</script>

<MobileDragDropPolyfill />

<PageShell
	title="จัดตารางสอน"
	description="จัดคาบตามกลุ่มเรียนหรือห้องประจำชั้น โดยใช้ตารางเวลาและรายการเปิดสอนของภาคเรียนที่เลือก"
>
	{#snippet actions()}
		{#if canManage && academicTermId && !dirty}
			{#if activeDraftVersion}
				<Button
					variant="outline"
					disabled={selectedVersion?.id === activeDraftVersion.id}
					onclick={() => changeVersion(activeDraftVersion.id)}
				>
					<History />
					{selectedVersion?.id === activeDraftVersion.id
						? 'กำลังแก้รุ่นแบบร่าง'
						: 'เปิดรุ่นแบบร่าง'}
				</Button>
			{:else if selectedVersion?.status === 'published'}
				<AcademicChangeSetDialog
					{academicTermId}
					purpose="timetable_revision"
					onCreated={handleRevisionCreated}
				/>
			{/if}
		{/if}
		<Button
			variant="outline"
			disabled={isTeacherLoadExporting || loading || entries.length === 0 || !academicTermId}
			onclick={handleExportTeacherLoadXlsx}
		>
			{#if isTeacherLoadExporting}
				<Loader2 class="animate-spin" />
			{:else}
				<FileSpreadsheet />
			{/if}
			สรุปคาบ XLSX
		</Button>
	{/snippet}

	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูตารางสอน"
			description="ต้องมีสิทธิ์อ่านรายการเปิดสอนที่เกี่ยวข้อง"
		/>
	{:else if !academicTermId || !academicYearId}
		<PageState
			variant="empty"
			title="เลือกภาคเรียนก่อน"
			description="ใช้ตัวเลือกปีการศึกษาและภาคเรียนบนแถบด้านบน"
		/>
	{:else if loading}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && entries.length === 0}
		<PageState
			variant="error"
			title="โหลดตารางสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(academicTermId, academicYearId)}
		/>
	{:else if versions.length === 0}
		<PageState
			variant="empty"
			title="ยังไม่มีรุ่นตารางสอน"
			description="ต้องเตรียมและเผยแพร่รุ่นตารางสอนของภาคเรียนนี้ก่อนจึงจะแสดงตารางได้"
		/>
	{:else}
		<div class="space-y-5">
			<Card.Root class="gap-0 overflow-hidden border-primary/20 bg-primary/[0.025] py-0">
				<Card.Content class="grid gap-4 p-4 lg:grid-cols-[minmax(15rem,1fr)_auto] lg:items-center">
					<div class="flex min-w-0 items-start gap-3">
						<div
							class="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary"
						>
							<History class="size-4" />
						</div>
						<div class="min-w-0">
							<div class="flex flex-wrap items-center gap-2">
								<p class="font-semibold">รุ่นตารางสอน</p>
								{#if selectedVersion}
									<Badge variant={selectedVersion.status === 'draft' ? 'secondary' : 'default'}>
										{versionStatusLabel(selectedVersion)}
									</Badge>
								{/if}
							</div>
							<p class="mt-1 text-xs text-muted-foreground">
								{#if selectedVersion}
									มีผล {versionPeriodLabel(selectedVersion)} · {selectedVersion.status === 'draft'
										? canManage
											? 'แก้ไขคาบได้'
											: 'อ่านอย่างเดียว คุณไม่มีสิทธิ์แก้ไขคาบ'
										: 'อ่านอย่างเดียว ข้อมูลที่เผยแพร่แล้วไม่ถูกแก้ย้อนหลัง'}
								{:else}
									เลือกรุ่นตารางสอน
								{/if}
							</p>
						</div>
					</div>
					<div class="w-full lg:w-80">
						<Select.Root
							type="single"
							bind:value={versionSelectValue}
							onValueChange={changeVersion}
						>
							<Select.Trigger class="w-full bg-background" aria-label="เลือกรุ่นตารางสอน">
								{selectedVersion
									? `${versionStatusLabel(selectedVersion)} · เริ่ม ${selectedVersion.effectiveFrom}`
									: 'เลือกรุ่นตารางสอน'}
							</Select.Trigger>
							<Select.Content>
								{#each versions as version (version.id)}
									<Select.Item value={version.id}>
										{versionStatusLabel(version)} · {versionPeriodLabel(version)}
									</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
				</Card.Content>
			</Card.Root>

			{#if selectedChangeSet}
				<Card.Root class="gap-0 overflow-hidden border-amber-500/25 py-0">
					<Card.Header class="border-b border-amber-500/20 bg-amber-500/5 py-4">
						<div class="flex flex-wrap items-start justify-between gap-3">
							<div>
								<Card.Title class="text-base">ขั้นตอนของรุ่นตารางสอนนี้</Card.Title>
								<Card.Description class="mt-1">{selectedChangeSet.reason}</Card.Description>
							</div>
							<Badge variant="outline" class="bg-background">
								เริ่มใช้ {selectedChangeSet.effectiveFrom}
							</Badge>
						</div>
					</Card.Header>
					<Card.Content class="p-4 sm:p-5">
						{#if dirty}
							<p
								class="rounded-xl border border-amber-500/30 bg-amber-500/8 p-3 text-sm text-amber-900"
							>
								บันทึกหรือยกเลิกคาบที่กำลังแก้ก่อนตรวจความพร้อมและเผยแพร่รุ่นนี้
							</p>
						{:else}
							{#key `${selectedChangeSet.id}:${draftRevision}`}
								<AcademicChangeReadiness
									changeSet={selectedChangeSet}
									{canManage}
									onChanged={handleChangeSetChanged}
								/>
							{/key}
						{/if}
					</Card.Content>
				</Card.Root>
			{/if}

			{#if groups.length === 0}
				<AcademicPrerequisiteNotice prerequisite={missingGroupsPrerequisite} />
			{/if}
			{#if groupsWithoutTeachers > 0}
				<AcademicPrerequisiteNotice prerequisite={missingTeachersPrerequisite} />
			{/if}
			{#if schedules.length === 0 || periods.length === 0}
				<AcademicPrerequisiteNotice prerequisite={missingPeriodsPrerequisite} />
			{/if}
			{#if rooms.length === 0}
				<AcademicPrerequisiteNotice prerequisite={missingRoomsPrerequisite} />
			{/if}

			<Card.Root class="gap-0 py-0">
				<Card.Content class="grid gap-4 pt-6 lg:grid-cols-[14rem_auto_minmax(16rem,1fr)]">
					<div class="space-y-2">
						<Label>ตารางเวลา</Label>
						<div class="flex h-9 items-center rounded-md border bg-muted/30 px-3 text-sm">
							{selectedSchedule
								? `${selectedSchedule.code} · ${selectedSchedule.name}`
								: 'ไม่พบตารางเวลา'}
						</div>
					</div>
					<div class="space-y-2">
						<Label>มุมมอง</Label>
						<div class="flex gap-2">
							<Button
								variant={viewKind === 'learning_group' ? 'default' : 'outline'}
								disabled={groups.length === 0}
								onclick={() => changeViewKind('learning_group')}>กลุ่มเรียน</Button
							>
							<Button
								variant={viewKind === 'homeroom' ? 'default' : 'outline'}
								onclick={() => changeViewKind('homeroom')}>ห้องประจำชั้น</Button
							>
						</div>
					</div>
					<div class="space-y-2">
						<Label for="timetable-target"
							>{viewKind === 'learning_group' ? 'กลุ่มเรียน' : 'ห้องประจำชั้น'}</Label
						>
						<Select.Root type="single" bind:value={targetSelectValue} onValueChange={changeTarget}>
							<Select.Trigger id="timetable-target" class="w-full">
								{#if viewKind === 'learning_group'}
									{@const group = groups.find((item) => item.id === selectedTargetId)}
									{group ? groupLabel(group) : 'เลือกกลุ่มเรียน'}
								{:else}
									{@const homeroom = homerooms.find((item) => item.id === selectedTargetId)}
									{homeroom ? `${homeroom.code} · ${homeroom.name}` : 'เลือกห้องประจำชั้น'}
								{/if}
							</Select.Trigger>
							<Select.Content>
								{#if viewKind === 'learning_group'}
									{#each groups as group (group.id)}
										<Select.Item value={group.id}>{groupLabel(group)}</Select.Item>
									{/each}
								{:else}
									{#each homerooms as homeroom (homeroom.id)}
										<Select.Item value={homeroom.id}>{homeroom.code} · {homeroom.name}</Select.Item>
									{/each}
								{/if}
							</Select.Content>
						</Select.Root>
					</div>
				</Card.Content>
			</Card.Root>

			{#if dirty}
				<p class="text-amber-700 dark:text-amber-300 text-sm">
					มีแบบร่างคาบที่ยังไม่บันทึก — ต้องบันทึกหรือยกเลิกก่อนเปลี่ยนบริบท
				</p>
			{/if}
			{#if errorMessage}
				<div
					class="border-destructive/30 bg-destructive/5 text-destructive rounded-lg border p-3 text-sm"
				>
					{errorMessage}
				</div>
			{/if}

			<div class="grid items-start gap-5 xl:grid-cols-[minmax(0,1.7fr)_22rem]">
				<Card.Root class="overflow-hidden">
					<Card.Header>
						<Card.Title class="flex items-center gap-2"
							><CalendarClock /> ตารางรายสัปดาห์</Card.Title
						>
						<Card.Description>
							{canEditSelected
								? 'คลิกช่องว่างเพื่อเพิ่มคาบ หรือคลิกคาบเดิมเพื่อแก้ไข'
								: selectedVersion?.status === 'draft'
									? 'คุณดูแบบร่างนี้ได้ แต่ไม่มีสิทธิ์เพิ่ม แก้ไข หรือลบคาบ'
									: 'รุ่นที่เผยแพร่แล้วเป็นข้อมูลอ่านอย่างเดียว'}
						</Card.Description>
					</Card.Header>
					<Card.Content class="overflow-x-auto">
						<table class="w-full min-w-[760px] border-separate border-spacing-0 text-sm">
							<thead>
								<tr>
									<th class="bg-muted/70 sticky left-0 z-10 border p-2 text-left">คาบ</th>
									{#each activeDays as day (day.value)}<th
											class="bg-muted/70 border p-2 text-center">{day.label}</th
										>{/each}
								</tr>
							</thead>
							<tbody>
								{#each periods as period (period.id)}
									<tr>
										<th
											class="bg-background sticky left-0 z-10 w-28 border p-2 text-left font-normal"
										>
											<p class="font-medium">{period.name ?? `คาบ ${period.orderIndex}`}</p>
											<p class="text-muted-foreground text-xs">
												{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
											</p>
										</th>
										{#each activeDays as day (day.value)}
											{@const cellEntries = entriesForCell(day.value, period.id)}
											<td class="h-24 min-w-32 border p-1 align-top">
												{#if cellEntries.length === 0}
													<button
														type="button"
														class="hover:bg-muted text-muted-foreground flex size-full min-h-20 items-center justify-center rounded-md border border-dashed"
														disabled={!canEditSelected || !selectedTargetId}
														onclick={() => startAtCell(day.value, period.id)}
														title="เพิ่มคาบ"><Plus class="size-4" /></button
													>
												{:else}
													<div class="space-y-1">
														{#each cellEntries as entry (entry.id)}
															<button
																type="button"
																class="bg-primary/10 hover:bg-primary/15 border-primary/20 w-full rounded-md border p-2 text-left {selectedEntryId ===
																entry.id
																	? 'ring-primary ring-2'
																	: ''}"
																onclick={() => editEntry(entry)}
															>
																<p class="truncate font-medium">{entryTitle(entry)}</p>
																<p class="text-muted-foreground mt-1 truncate text-xs">
																	{entry.roomCode ??
																		entry.learningGroupCode ??
																		entry.homeroomName ??
																		'ไม่ระบุห้อง'}
																</p>
																{#if entry.instructors.length > 0}
																	<p class="mt-1 truncate text-[0.7rem] text-primary/80">
																		{entry.instructors
																			.map((instructor) => instructor.displayName)
																			.join(', ')}
																	</p>
																{/if}
															</button>
														{/each}
													</div>
												{/if}
											</td>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					</Card.Content>
				</Card.Root>

				<Card.Root>
					<Card.Header>
						<div class="flex items-center justify-between gap-2">
							<div>
								<Card.Title>
									{canEditSelected ? (selectedEntry ? 'แก้ไขคาบ' : 'เพิ่มคาบ') : 'รายละเอียดคาบ'}
								</Card.Title>
								<Card.Description
									>{selectedEntry
										? `เวอร์ชัน ${selectedEntry.rowVersion}`
										: 'เลือกวันและคาบที่ต้องการ'}</Card.Description
								>
							</div>
							{#if dirty}<Badge variant="secondary">ยังไม่บันทึก</Badge>{/if}
						</div>
					</Card.Header>
					<Card.Content class="space-y-4">
						<div class="grid grid-cols-2 gap-3">
							<div class="space-y-2">
								<Label for="entry-day">วัน</Label>
								<Select.Root
									type="single"
									bind:value={formDay}
									disabled={!canEditSelected}
									onValueChange={markDirty}
								>
									<Select.Trigger id="entry-day" class="w-full">
										{activeDays.find((day) => day.value === formDay)?.label ?? 'เลือกวัน'}
									</Select.Trigger>
									<Select.Content>
										{#each activeDays as day (day.value)}
											<Select.Item value={day.value}>{day.label}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
							<div class="space-y-2">
								<Label for="entry-period">คาบ</Label>
								<Select.Root
									type="single"
									bind:value={formPeriodId}
									disabled={!canEditSelected}
									onValueChange={markDirty}
								>
									<Select.Trigger id="entry-period" class="w-full">
										{periods.find((period) => period.id === formPeriodId)?.name ??
											periods.find((period) => period.id === formPeriodId)?.orderIndex ??
											'เลือกคาบ'}
									</Select.Trigger>
									<Select.Content>
										{#each periods as period (period.id)}
											<Select.Item value={period.id}>{period.name ?? period.orderIndex}</Select.Item
											>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
						</div>
						<div class="space-y-2">
							<Label for="entry-type">ประเภทคาบ</Label>
							<Select.Root
								type="single"
								bind:value={formEntryType}
								disabled={!canEditSelected || Boolean(selectedEntry)}
								onValueChange={changeEntryType}
							>
								<Select.Trigger id="entry-type" class="w-full">
									{entryTypeOptions.find((option) => option.value === formEntryType)?.label ??
										'เลือกประเภทคาบ'}
								</Select.Trigger>
								<Select.Content>
									{#each entryTypeOptions as option (option.value)}
										<Select.Item value={option.value}>{option.label}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
						{#if canEditSelected && showsInstructorPicker}
							<TimetableInstructorPicker
								options={selectedGroupInstructorOptions}
								bind:value={
									() => formInstructorIds,
									(value) => {
										formInstructorIds = value;
										markDirty();
									}
								}
								disabled={busy}
								label="ครูผู้สอนของคาบนี้"
							/>
							{#if unavailableSelectedInstructors.length > 0}
								<div
									class="space-y-3 rounded-lg border border-amber-500/35 bg-amber-500/8 p-3 text-amber-950 dark:text-amber-100"
									role="alert"
								>
									<div class="flex items-start gap-2">
										<AlertTriangle class="mt-0.5 size-4 shrink-0" />
										<div>
											<p class="text-sm font-medium">มีครูเดิมที่ใช้กับรุ่นนี้ไม่ได้</p>
											<p class="mt-0.5 text-xs text-amber-900/80 dark:text-amber-100/75">
												รายชื่อต่อไปนี้ไม่ได้อยู่ในช่วงวันที่ของรุ่นตารางสอนนี้
												ต้องนำออกแล้วเลือกครูที่มีผลก่อนบันทึก
											</p>
										</div>
									</div>
									<div class="flex flex-wrap gap-2">
										{#each unavailableSelectedInstructors as instructor (instructor.userId)}
											<Button
												type="button"
												size="sm"
												variant="outline"
												class="h-auto border-amber-500/40 bg-background py-1.5"
												aria-label={`นำ ${instructor.displayName} ออกจากคาบ`}
												onclick={() => removeUnavailableInstructor(instructor.userId)}
											>
												นำ {instructor.displayName} ออก
											</Button>
										{/each}
									</div>
								</div>
							{/if}
						{:else if selectedEntry && !canEditSelected}
							<div class="space-y-2">
								<Label>ครูผู้สอนของคาบนี้</Label>
								{#if selectedEntry.instructors.length === 0}
									<p
										class="rounded-lg border border-dashed px-3 py-2 text-sm text-muted-foreground"
									>
										ยังไม่ได้ระบุครูผู้สอนสำหรับคาบนี้
									</p>
								{:else}
									<div class="flex flex-wrap gap-2">
										{#each selectedEntry.instructors as instructor (instructor.userId)}
											<Badge variant="outline" class="gap-1.5 py-1">
												{instructor.displayName}
												<span class="text-muted-foreground text-[0.7rem]">
													{instructorRoleLabel(instructor.role)}
												</span>
											</Badge>
										{/each}
									</div>
								{/if}
							</div>
						{/if}
						<div class="space-y-2">
							<Label for="entry-room">ห้องเรียน</Label>
							<Select.Root
								type="single"
								value={formRoomId || NO_ROOM_VALUE}
								disabled={!canEditSelected}
								onValueChange={(value) => {
									formRoomId = value === NO_ROOM_VALUE ? '' : value;
									markDirty();
								}}
							>
								<Select.Trigger id="entry-room" class="w-full">
									{@const room = rooms.find((item) => item.id === formRoomId)}
									{room ? `${room.code ? `${room.code} · ` : ''}${room.name_th}` : 'ไม่ระบุ'}
								</Select.Trigger>
								<Select.Content>
									<Select.Item value={NO_ROOM_VALUE}>ไม่ระบุ</Select.Item>
									{#each rooms as room (room.id)}
										<Select.Item value={room.id}
											>{room.code ? `${room.code} · ` : ''}{room.name_th}</Select.Item
										>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
						<div class="space-y-2">
							<Label for="entry-title">ชื่อแสดงเพิ่มเติม</Label>
							<Input
								id="entry-title"
								bind:value={formTitle}
								disabled={!canEditSelected}
								oninput={markDirty}
							/>
						</div>
						<div class="space-y-2">
							<Label for="entry-note">หมายเหตุ</Label>
							<Input
								id="entry-note"
								bind:value={formNote}
								disabled={!canEditSelected}
								oninput={markDirty}
							/>
						</div>
						{#if canEditSelected}
							<div class="flex flex-wrap gap-2 pt-2">
								<Button
									disabled={busy ||
										!dirty ||
										!selectedTargetId ||
										!formPeriodId ||
										unavailableSelectedInstructors.length > 0}
									onclick={saveEntry}
									>{#if busy}<Loader2 class="animate-spin" />{:else}<Save />{/if} บันทึก</Button
								>
								<Button variant="outline" disabled={busy || !dirty} onclick={resetForm}
									><RotateCcw /> ยกเลิก</Button
								>
								{#if selectedEntry}<Button
										variant="destructive"
										class="ml-auto"
										disabled={busy}
										onclick={removeEntry}><Trash2 /> ลบคาบ</Button
									>{/if}
							</div>
						{/if}
					</Card.Content>
				</Card.Root>
			</div>
		</div>
	{/if}
</PageShell>
