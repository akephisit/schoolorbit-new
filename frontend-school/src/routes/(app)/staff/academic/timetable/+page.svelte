<script lang="ts">
	import { replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';

	import { getAcademicContextStore } from '$lib/academic-context/store';
	import {
		entriesForTimetableCell,
		localPlacementPreview,
		type TimetableBoardView
	} from '$lib/academic/timetable/board-state';
	import {
		createTimetableWorkspaceController,
		type TimetableDragSource,
		type TimetableWorkspaceController
	} from '$lib/academic/timetable/workspace-controller.svelte';
	import { ApiClientError } from '$lib/api/client';
	import { getAcademicTermChangeSet, type AcademicTermChangeSet } from '$lib/api/learning-delivery';
	import {
		createTimetableEntry,
		currentLocalDate,
		deleteTimetableEntry,
		getTimetableWorkspace,
		listTimetableVersions,
		previewTimetablePlacement,
		swapTimetableEntries,
		updateTimetableEntry,
		type TimetableEntry,
		type TimetablePlacementCandidate,
		type TimetablePlacementPreview,
		type TimetablePlacementPreviewRequest,
		type TimetablePlacementSource,
		type TimetableVersion,
		type TimetableWorkspace
	} from '$lib/api/timetable';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import TimetableBoard from '$lib/components/academic/timetable/TimetableBoard.svelte';
	import type { TimetableCellState } from '$lib/components/academic/timetable/TimetableCell.svelte';
	import TimetableEntryInspector from '$lib/components/academic/timetable/TimetableEntryInspector.svelte';
	import TimetableInstructorPicker from '$lib/components/academic/timetable/TimetableInstructorPicker.svelte';
	import TimetableMoveDialog from '$lib/components/academic/timetable/TimetableMoveDialog.svelte';
	import TimetableUnscheduledTray from '$lib/components/academic/timetable/TimetableUnscheduledTray.svelte';
	import TimetableWorkspaceHeader from '$lib/components/academic/timetable/TimetableWorkspaceHeader.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import {
		AcademicPrerequisiteNotice,
		type AcademicPrerequisite
	} from '$lib/components/academic-workflow';
	import AcademicChangeReadiness from '$lib/components/learning-delivery/AcademicChangeReadiness.svelte';
	import AcademicChangeSetDialog from '$lib/components/learning-delivery/AcademicChangeSetDialog.svelte';
	import MobileDragDropPolyfill from '$lib/components/MobileDragDropPolyfill.svelte';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import {
		connectTimetableSocket,
		disconnectTimetableSocket,
		refreshTrigger
	} from '$lib/stores/timetable-socket';
	import { AlertTriangle, History, LoaderCircle, MousePointer2, RefreshCw } from 'lucide-svelte';

	type InstructorOption = {
		id: string;
		displayName: string;
		role: 'primary' | 'secondary' | 'assistant';
	};

	const missingGroupsPrerequisite: AcademicPrerequisite = {
		key: 'timetable-learning-groups',
		status: 'missing',
		title: 'ยังไม่มีกลุ่มเรียนสำหรับจัดตาราง',
		description: 'สร้างกลุ่มใต้รายการเปิดสอน และกำหนดห้องต้นทางให้ตรงกับการสอนจริง',
		actionLabel: 'ไปจัดกลุ่มเรียน',
		href: '/staff/academic/delivery'
	};
	const missingTeachersPrerequisite: AcademicPrerequisite = {
		key: 'timetable-teachers',
		status: 'warning',
		title: 'บางกลุ่มยังไม่มีครูผู้สอน',
		description: 'กำหนดครูในหน้าจัดการเรียนก่อนวางคาบ เพื่อให้ตรวจตารางชนได้ถูกต้อง',
		actionLabel: 'ไปกำหนดครู',
		href: '/staff/academic/delivery'
	};
	const missingPeriodsPrerequisite: AcademicPrerequisite = {
		key: 'timetable-periods',
		status: 'missing',
		title: 'ยังไม่มีคาบเรียนในตารางเวลา',
		description: 'ตั้งเวลาเริ่มและสิ้นสุดของแต่ละคาบก่อนจัดตารางสอน',
		actionLabel: 'ไปตั้งค่าคาบเรียน',
		href: '/staff/academic/core#bell-schedules'
	};
	const missingRoomsPrerequisite: AcademicPrerequisite = {
		key: 'timetable-rooms',
		status: 'warning',
		title: 'ยังไม่มีห้องเรียนให้เลือก',
		description: 'เพิ่มอาคารและห้องเรียนก่อนกำหนดห้องเฉพาะให้คาบ',
		actionLabel: 'ไปจัดการห้อง',
		href: '/staff/facility/buildings'
	};

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const academicYearId = $derived($academicContext.selected.academicYearId);
	const request = new LatestRequest();

	let versions = $state<TimetableVersion[]>([]);
	let controller = $state.raw<TimetableWorkspaceController | null>(null);
	let selectedChangeSet = $state.raw<AcademicTermChangeSet | null>(null);
	let loading = $state(false);
	let busy = $state(false);
	let previewing = $state(false);
	let errorMessage = $state('');
	let draftRevision = $state(0);
	let inspectorOpen = $state(false);
	let inspectorEntryId = $state<string | null>(null);
	let moveDialogOpen = $state(false);
	let moveEntryId = $state<string | null>(null);
	let removeDialogOpen = $state(false);
	let removeEntryId = $state<string | null>(null);
	let teacherDialogOpen = $state(false);
	let pendingDemandSource = $state.raw<TimetablePlacementSource | null>(null);
	let pendingDemandCandidate = $state.raw<TimetablePlacementCandidate | null>(null);
	let pendingDemandInstructorIds = $state<string[]>([]);

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
	const selectedVersion = $derived(controller?.workspace.version ?? null);
	let versionSelectValue = $derived(selectedVersion?.id ?? '');
	const activeDraftVersion = $derived(
		versions.find((version) => version.status === 'draft' && version.changeSetId) ?? null
	);
	const canEditSelected = $derived(
		Boolean(canManage && controller?.canEdit && !busy && !previewing)
	);
	const groupsWithoutTeachers = $derived(
		controller?.workspace.learningGroups.filter((group) => group.eligibleInstructorIds.length === 0)
			.length ?? 0
	);
	const selectedEntry = $derived(
		controller?.workspace.entries.find((entry) => entry.id === inspectorEntryId) ?? null
	);
	const moveEntry = $derived(
		controller?.workspace.entries.find((entry) => entry.id === moveEntryId) ?? null
	);
	const removeEntry = $derived(
		controller?.workspace.entries.find((entry) => entry.id === removeEntryId) ?? null
	);
	const selectedOwner = $derived(
		controller?.rows.find((row) => row.id === controller?.selectedOwnerId) ?? null
	);
	const visibleDemands = $derived.by(() => {
		if (!controller || !controller.selectedOwnerId) return [];
		const current = controller;
		const ownerId = controller.selectedOwnerId;
		if (current.view === 'learning_group') {
			return current.workspace.unscheduledDemands.filter(
				(demand) => demand.learningGroupId === ownerId
			);
		}
		const groupIds = new Set(
			current.workspace.learningGroups
				.filter((group) => group.homeroomIds.includes(ownerId))
				.map((group) => group.id)
		);
		return current.workspace.unscheduledDemands.filter((demand) =>
			groupIds.has(demand.learningGroupId)
		);
	});
	const pendingDemandInstructorOptions = $derived(
		pendingDemandCandidate?.learningGroupId
			? instructorOptionsForGroup(pendingDemandCandidate.learningGroupId)
			: []
	);

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
		return upcoming ?? loadedVersions.find((version) => version.status === 'draft') ?? null;
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

	function requestedView(): TimetableBoardView {
		return page.url.searchParams.get('view') === 'learningGroup' ? 'learning_group' : 'homeroom';
	}

	function syncUrl(): void {
		if (!controller) return;
		const nextUrl = new URL(page.url);
		nextUrl.searchParams.set('timetableVersionId', controller.workspace.version.id);
		nextUrl.searchParams.set(
			'view',
			controller.view === 'learning_group' ? 'learningGroup' : 'homeroom'
		);
		if (controller.selectedOwnerId) nextUrl.searchParams.set('ownerId', controller.selectedOwnerId);
		else nextUrl.searchParams.delete('ownerId');
		replaceState(
			resolve(`/staff/academic/timetable?${nextUrl.searchParams.toString()}`),
			page.state
		);
	}

	function initializeController(workspace: TimetableWorkspace): TimetableWorkspaceController {
		const next = createTimetableWorkspaceController(workspace);
		next.setView(requestedView());
		const ownerId = page.url.searchParams.get('ownerId');
		if (ownerId) next.selectOwner(ownerId);
		return next;
	}

	function fetchWorkspace(
		yearId: string,
		termId: string,
		versionId: string,
		signal?: AbortSignal
	): Promise<TimetableWorkspace> {
		return getTimetableWorkspace(
			{
				academicYearId: yearId,
				academicTermId: termId,
				timetableVersionId: versionId
			},
			{ signal }
		);
	}

	async function loadChangeSet(
		version: TimetableVersion,
		signal?: AbortSignal
	): Promise<AcademicTermChangeSet | null> {
		return version.changeSetId
			? getAcademicTermChangeSet(version.changeSetId, { signal })
			: Promise.resolve(null);
	}

	async function loadWorkspaceContext(termId: string, yearId: string): Promise<void> {
		const { revision, signal } = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const loadedVersions = await listTimetableVersions(termId, { signal });
			const preferred = selectPreferredVersion(loadedVersions);
			if (!preferred) {
				if (request.isCurrent(revision)) {
					versions = loadedVersions;
					controller = null;
				}
				return;
			}
			const workspace = await fetchWorkspace(yearId, termId, preferred.id, signal);
			const changeSet = await loadChangeSet(workspace.version, signal);
			if (!request.isCurrent(revision)) return;
			versions = loadedVersions;
			controller = initializeController(workspace);
			selectedChangeSet = changeSet;
			draftRevision += 1;
			syncUrl();
		} catch (error) {
			if (!isAbortError(error) && request.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดพื้นที่จัดตารางสอนไม่สำเร็จ';
			}
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function changeVersion(versionId: string, force = false): Promise<void> {
		if (!academicYearId || !academicTermId || (!force && versionId === selectedVersion?.id)) return;
		const version = versions.find((item) => item.id === versionId);
		if (!version) return;
		const { revision, signal } = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const workspace = await fetchWorkspace(academicYearId, academicTermId, version.id, signal);
			const changeSet = await loadChangeSet(workspace.version, signal);
			if (!request.isCurrent(revision)) return;
			controller = initializeController(workspace);
			selectedChangeSet = changeSet;
			draftRevision += 1;
			syncUrl();
		} catch (error) {
			if (!isAbortError(error) && request.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดรุ่นตารางสอนไม่สำเร็จ';
				toast.error(errorMessage);
			}
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function reloadSelectedWorkspace(message?: string): Promise<void> {
		if (!controller || !academicYearId || !academicTermId) return;
		const versionId = controller.workspace.version.id;
		controller.setRefreshing(true);
		try {
			const workspace = await fetchWorkspace(academicYearId, academicTermId, versionId);
			controller.setWorkspace(workspace);
			controller.clearPlacement();
			draftRevision += 1;
			if (message) toast.info(message);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดข้อมูลตารางล่าสุดไม่สำเร็จ';
			toast.error(errorMessage);
		} finally {
			controller.setRefreshing(false);
		}
	}

	function changeView(view: TimetableBoardView): void {
		if (!controller) return;
		controller.setView(view);
		syncUrl();
	}

	function changeOwner(ownerId: string): void {
		if (!controller) return;
		controller.selectOwner(ownerId);
		controller.clearPlacement();
		syncUrl();
	}

	function candidateForEntry(
		entry: TimetableEntry,
		overrides?: { roomId: string | null; instructorIds: string[] }
	): TimetablePlacementCandidate {
		return {
			entryType: entry.entryType,
			learningGroupId: entry.learningGroupId ?? null,
			learningOfferingId: entry.offeringId ?? null,
			homeroomId: entry.homeroomId ?? null,
			roomId: overrides?.roomId ?? entry.roomId ?? null,
			instructorIds:
				overrides?.instructorIds ?? entry.instructors.map((instructor) => instructor.userId)
		};
	}

	function startExistingPlacement(entry: TimetableEntry): void {
		controller?.startPlacement(
			{ kind: 'existing_entry', entryId: entry.id, rowVersion: entry.rowVersion },
			candidateForEntry(entry)
		);
	}

	function cancelDrag(): void {
		if (!previewing) controller?.clearPlacement();
	}

	function chooseDemand(
		source: TimetablePlacementSource,
		candidate: TimetablePlacementCandidate
	): void {
		if (!controller) return;
		if (candidate.instructorIds.length === 1) {
			controller.startPlacement(source, candidate);
			toast.info('เลือกช่องสีเขียวเพื่อวาง 1 คาบ');
			return;
		}
		pendingDemandSource = source;
		pendingDemandCandidate = candidate;
		pendingDemandInstructorIds = [];
		teacherDialogOpen = true;
	}

	function startDemandAfterTeacherSelection(): void {
		if (
			!controller ||
			!pendingDemandSource ||
			!pendingDemandCandidate ||
			pendingDemandInstructorIds.length === 0
		)
			return;
		controller.startPlacement(pendingDemandSource, {
			...pendingDemandCandidate,
			instructorIds: pendingDemandInstructorIds
		});
		teacherDialogOpen = false;
		toast.info('เลือกช่องสีเขียวเพื่อวาง 1 คาบ');
	}

	function cellState(dayOfWeek: string, periodId: string): TimetableCellState {
		if (!controller?.dragSource || !controller.selectedOwnerId) return 'neutral';
		const source = controller.dragSource.source;
		if (source.kind === 'existing_entry') {
			const entry = controller.board.entriesById.get(source.entryId);
			if (entry?.dayOfWeek === dayOfWeek && entry.bellSchedulePeriodId === periodId) {
				return 'dragging';
			}
		}
		if (
			controller.preview?.targetDayOfWeek === dayOfWeek &&
			controller.preview.targetBellSchedulePeriodId === periodId
		) {
			return controller.preview.state === 'source' ? 'dragging' : controller.preview.state;
		}
		const local = localPlacementPreview(controller.board, {
			view: controller.view,
			rowId: controller.selectedOwnerId,
			dayOfWeek,
			bellSchedulePeriodId: periodId,
			source,
			candidate: controller.dragSource.candidate
		}).state;
		return local === 'source' ? 'dragging' : local;
	}

	function targetEntryFor(dayOfWeek: string, periodId: string): TimetableEntry | null {
		if (!controller?.selectedRow) return null;
		const sourceEntryId =
			controller.dragSource?.source.kind === 'existing_entry'
				? controller.dragSource.source.entryId
				: null;
		return (
			entriesForTimetableCell(controller.board, {
				view: controller.view,
				rowId: controller.selectedRow.id,
				dayOfWeek,
				bellSchedulePeriodId: periodId
			}).find((entry) => entry.id !== sourceEntryId) ?? null
		);
	}

	function patchEntries(changedEntries: TimetableEntry[]): void {
		if (!controller) return;
		const changedById = new Map(changedEntries.map((entry) => [entry.id, entry]));
		const existingIds = new Set(controller.workspace.entries.map((entry) => entry.id));
		const nextEntries = controller.workspace.entries
			.map((entry) => changedById.get(entry.id) ?? entry)
			.filter((entry) => entry.isActive)
			.concat(changedEntries.filter((entry) => entry.isActive && !existingIds.has(entry.id)));
		controller.setWorkspace({ ...controller.workspace, entries: nextEntries });
		draftRevision += 1;
	}

	async function applyPlacementPreview(
		preview: TimetablePlacementPreview,
		dragSource: TimetableDragSource
	): Promise<boolean> {
		if (!controller || !preview.mutation || !academicTermId) return false;
		controller.beginMutation(preview.mutation);
		busy = true;
		try {
			if (preview.mutation === 'create') {
				const created = await createTimetableEntry({
					academicTermId,
					timetableVersionId: controller.workspace.version.id,
					learningGroupId: preview.normalizedCandidate.learningGroupId,
					homeroomId: preview.normalizedCandidate.homeroomId,
					dayOfWeek: preview.targetDayOfWeek,
					bellSchedulePeriodId: preview.targetBellSchedulePeriodId,
					roomId: preview.normalizedCandidate.roomId,
					note: null,
					entryType: preview.normalizedCandidate.entryType,
					title: null,
					instructorIds: preview.normalizedCandidate.instructorIds
				});
				patchEntries([created]);
			} else if (preview.mutation === 'swap') {
				if (dragSource.source.kind !== 'existing_entry' || !preview.targetEntryId) {
					throw new Error('ข้อมูลสำหรับสลับคาบไม่ครบ');
				}
				const targetEntry = controller.board.entriesById.get(preview.targetEntryId);
				if (!targetEntry) throw new Error('ไม่พบคาบปลายทาง กรุณาโหลดข้อมูลล่าสุด');
				const swapped = await swapTimetableEntries({
					timetableVersionId: controller.workspace.version.id,
					entryAId: dragSource.source.entryId,
					entryARowVersion: dragSource.source.rowVersion,
					entryBId: targetEntry.id,
					entryBRowVersion: targetEntry.rowVersion
				});
				patchEntries([swapped.entryA, swapped.entryB]);
			} else {
				if (dragSource.source.kind !== 'existing_entry') {
					throw new Error('ข้อมูลสำหรับย้ายคาบไม่ครบ');
				}
				const updated = await updateTimetableEntry(dragSource.source.entryId, {
					timetableVersionId: controller.workspace.version.id,
					rowVersion: dragSource.source.rowVersion,
					dayOfWeek: preview.targetDayOfWeek,
					bellSchedulePeriodId: preview.targetBellSchedulePeriodId,
					roomId: preview.normalizedCandidate.roomId,
					clearRoom: preview.normalizedCandidate.roomId === null,
					instructorIds: preview.normalizedCandidate.instructorIds
				});
				patchEntries([updated]);
			}
			controller.finishMutation();
			toast.success(preview.mutation === 'swap' ? 'สลับคาบแล้ว' : 'บันทึกตำแหน่งคาบแล้ว');
			return true;
		} catch (error) {
			controller.failMutation();
			if (error instanceof ApiClientError && error.status === 409) {
				await reloadSelectedWorkspace('มีผู้ใช้อื่นเปลี่ยนตาราง ระบบโหลดข้อมูลล่าสุดให้แล้ว');
				return false;
			}
			errorMessage = error instanceof Error ? error.message : 'บันทึกตำแหน่งคาบไม่สำเร็จ';
			toast.error(errorMessage);
			return false;
		} finally {
			busy = false;
		}
	}

	async function attemptPlacement(
		dayOfWeek: string,
		periodId: string,
		dragSource: TimetableDragSource | null = controller?.dragSource ?? null
	): Promise<boolean> {
		if (!controller || !dragSource || !academicTermId || busy || previewing) return false;
		const targetEntry = targetEntryFor(dayOfWeek, periodId);
		const payload = {
			timetableVersionId: controller.workspace.version.id,
			academicTermId,
			source: dragSource.source,
			candidate: dragSource.candidate,
			targetDayOfWeek: dayOfWeek,
			targetBellSchedulePeriodId: periodId,
			expectedTargetEntryId: targetEntry?.id ?? null,
			expectedTargetRowVersion: targetEntry?.rowVersion ?? null
		} satisfies TimetablePlacementPreviewRequest;
		previewing = true;
		try {
			const preview = await previewTimetablePlacement(payload);
			controller.setPreview(preview);
			if (preview.state === 'blocked' || !preview.mutation) {
				toast.error(
					preview.conflicts.map((conflict) => conflict.message).join(' · ') ||
						'วางคาบในตำแหน่งนี้ไม่ได้'
				);
				return false;
			}
			return await applyPlacementPreview(preview, dragSource);
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await reloadSelectedWorkspace('ข้อมูลตารางเปลี่ยนแล้ว ระบบโหลดข้อมูลล่าสุดให้แล้ว');
				return false;
			}
			errorMessage = error instanceof Error ? error.message : 'ตรวจสอบตำแหน่งคาบไม่สำเร็จ';
			toast.error(errorMessage);
			return false;
		} finally {
			previewing = false;
		}
	}

	function inspectEntry(entry: TimetableEntry): void {
		inspectorEntryId = entry.id;
		inspectorOpen = true;
	}

	function openMoveDialog(entry: TimetableEntry): void {
		inspectorOpen = false;
		moveEntryId = entry.id;
		moveDialogOpen = true;
	}

	async function confirmMove(dayOfWeek: string, periodId: string): Promise<void> {
		if (!moveEntry || !controller) return;
		const source: TimetableDragSource = {
			source: {
				kind: 'existing_entry',
				entryId: moveEntry.id,
				rowVersion: moveEntry.rowVersion
			},
			candidate: candidateForEntry(moveEntry)
		};
		controller.startPlacement(source.source, source.candidate);
		if (await attemptPlacement(dayOfWeek, periodId, source)) moveDialogOpen = false;
	}

	async function saveEntryDetails(value: {
		roomId: string | null;
		instructorIds: string[];
	}): Promise<void> {
		if (!selectedEntry || !controller) return;
		const source: TimetableDragSource = {
			source: {
				kind: 'existing_entry',
				entryId: selectedEntry.id,
				rowVersion: selectedEntry.rowVersion
			},
			candidate: candidateForEntry(selectedEntry, value)
		};
		controller.startPlacement(source.source, source.candidate);
		if (
			await attemptPlacement(selectedEntry.dayOfWeek, selectedEntry.bellSchedulePeriodId, source)
		) {
			inspectorOpen = false;
		}
	}

	function requestRemove(entry: TimetableEntry): void {
		inspectorOpen = false;
		removeEntryId = entry.id;
		removeDialogOpen = true;
	}

	async function confirmRemove(): Promise<void> {
		if (!removeEntry || !controller || busy) return;
		busy = true;
		try {
			const deleted = await deleteTimetableEntry(
				removeEntry.id,
				removeEntry.rowVersion,
				controller.workspace.version.id
			);
			patchEntries([deleted]);
			controller.clearPlacement();
			removeDialogOpen = false;
			toast.success('นำคาบออกจากตารางแล้ว');
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await reloadSelectedWorkspace('คาบนี้ถูกเปลี่ยนแล้ว ระบบโหลดข้อมูลล่าสุดให้แล้ว');
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'นำคาบออกจากตารางไม่สำเร็จ';
			toast.error(errorMessage);
		} finally {
			busy = false;
		}
	}

	function normalizedInstructorRole(
		role: string | undefined,
		index: number
	): InstructorOption['role'] {
		if (role === 'primary' || role === 'secondary' || role === 'assistant') return role;
		return index === 0 ? 'primary' : 'secondary';
	}

	function instructorOptionsForGroup(
		groupId: string,
		entry: TimetableEntry | null = null
	): InstructorOption[] {
		if (!controller) return [];
		const group = controller.workspace.learningGroups.find((item) => item.id === groupId);
		if (!group) return [];
		const ids = [
			...group.eligibleInstructorIds,
			...(entry?.instructors.map((instructor) => instructor.userId) ?? []).filter(
				(id) => !group.eligibleInstructorIds.includes(id)
			)
		];
		return ids.map((id, index) => {
			const staff = controller?.workspace.staff.find((item) => item.id === id);
			const currentInstructor = entry?.instructors.find((item) => item.userId === id);
			return {
				id,
				displayName: staff?.displayName ?? currentInstructor?.displayName ?? 'ครูที่อ้างอิง',
				role: normalizedInstructorRole(currentInstructor?.role, index)
			};
		});
	}

	function instructorOptionsForEntry(entry: TimetableEntry | null): InstructorOption[] {
		if (!entry) return [];
		if (entry.learningGroupId) return instructorOptionsForGroup(entry.learningGroupId, entry);
		return entry.instructors.map((instructor, index) => ({
			id: instructor.userId,
			displayName: instructor.displayName,
			role: normalizedInstructorRole(instructor.role, index)
		}));
	}

	async function handleRevisionCreated(created: AcademicTermChangeSet): Promise<void> {
		if (!academicTermId) return;
		selectedChangeSet = created;
		versions = await listTimetableVersions(academicTermId);
		await changeVersion(created.targetTimetableVersionId);
		toast.success('สร้างรุ่นตารางสอนแบบร่างแล้ว');
	}

	async function handleChangeSetChanged(updated: AcademicTermChangeSet): Promise<void> {
		selectedChangeSet = updated;
		if (!academicTermId) return;
		versions = await listTimetableVersions(academicTermId);
		await changeVersion(updated.targetTimetableVersionId, true);
		if (updated.status === 'published') toast.success('เผยแพร่รุ่นตารางสอนใหม่แล้ว');
		if (updated.status === 'cancelled') toast.success('ยกเลิกรุ่นตารางสอนแบบร่างแล้ว');
	}

	onMount(() => {
		let selectedTermId = '';
		let selectedYearId = '';
		let currentUserId = '';
		let loadedContextKey = '';
		let connectedSocketKey = '';
		let observedRefresh: number | null = null;

		function synchronizeContext(): void {
			if (!selectedTermId || !selectedYearId) return;
			const contextKey = `${selectedYearId}:${selectedTermId}`;
			if (contextKey !== loadedContextKey) {
				loadedContextKey = contextKey;
				controller = null;
				versions = [];
				selectedChangeSet = null;
				void loadWorkspaceContext(selectedTermId, selectedYearId);
			}
			if (!currentUserId) return;
			const socketKey = `${selectedTermId}:${currentUserId}`;
			if (socketKey !== connectedSocketKey) {
				connectedSocketKey = socketKey;
				connectTimetableSocket({ academicTermId: selectedTermId, currentUserId });
			}
		}

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
			void reloadSelectedWorkspace('ตารางถูกแก้จากอีกหน้าหนึ่ง ระบบโหลดข้อมูลล่าสุดแล้ว');
		});

		return () => {
			request.abort();
			unsubscribeRefresh();
			unsubscribeAuth();
			unsubscribeContext();
			disconnectTimetableSocket();
		};
	});
</script>

<MobileDragDropPolyfill />

<PageShell
	title="จัดตารางสอน"
	description="จัดครั้งละหนึ่งคาบในมุมมองห้องประจำชั้นหรือกลุ่มเรียน ระบบตรวจห้อง ครู และผู้เรียนชนกันก่อนบันทึก"
>
	{#snippet actions()}
		{#if canManage && academicTermId}
			{#if activeDraftVersion}
				<Button
					variant="outline"
					disabled={selectedVersion?.id === activeDraftVersion.id || loading}
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
			disabled={loading || !controller}
			onclick={() => reloadSelectedWorkspace()}
		>
			<RefreshCw class={controller?.isRefreshing ? 'animate-spin' : ''} /> โหลดล่าสุด
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
	{:else if loading && !controller}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && !controller}
		<PageState
			variant="error"
			title="โหลดตารางสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspaceContext(academicTermId, academicYearId)}
		/>
	{:else if versions.length === 0 || !controller}
		<PageState
			variant="empty"
			title="ยังไม่มีรุ่นตารางสอน"
			description="เตรียมรุ่นตารางสอนของภาคเรียนนี้ก่อนเริ่มจัดคาบ"
		/>
	{:else}
		<div class="space-y-5">
			<TimetableWorkspaceHeader
				version={controller.workspace.version}
				view={controller.view}
				isSaving={busy || previewing || controller.pendingMutation !== null}
				isRefreshing={controller.isRefreshing}
				onViewChange={changeView}
			/>

			<Card.Root class="gap-0 py-0">
				<Card.Content class="grid gap-3 p-3 sm:p-4 lg:grid-cols-2">
					<div class="space-y-1.5">
						<p class="text-xs font-medium text-muted-foreground">รุ่นตารางสอน</p>
						<Select.Root
							type="single"
							bind:value={versionSelectValue}
							onValueChange={changeVersion}
							disabled={loading || busy}
						>
							<Select.Trigger class="w-full" aria-label="เลือกรุ่นตารางสอน">
								{versionStatusLabel(controller.workspace.version)} · {versionPeriodLabel(
									controller.workspace.version
								)}
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
					<div class="space-y-1.5">
						<p class="text-xs font-medium text-muted-foreground">
							{controller.view === 'homeroom' ? 'ห้องประจำชั้น' : 'กลุ่มเรียน'}
						</p>
						<Select.Root
							type="single"
							value={controller.selectedOwnerId ?? ''}
							onValueChange={changeOwner}
							disabled={busy}
						>
							<Select.Trigger class="w-full" aria-label="เลือกรายการที่ต้องการจัด">
								{selectedOwner ? `${selectedOwner.code} · ${selectedOwner.label}` : 'เลือกรายการ'}
							</Select.Trigger>
							<Select.Content>
								{#each controller.rows as row (row.id)}
									<Select.Item value={row.id}>{row.code} · {row.label}</Select.Item>
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
						{#key `${selectedChangeSet.id}:${draftRevision}`}
							<AcademicChangeReadiness
								changeSet={selectedChangeSet}
								{canManage}
								onChanged={handleChangeSetChanged}
							/>
						{/key}
					</Card.Content>
				</Card.Root>
			{/if}

			{#if controller.workspace.learningGroups.length === 0}
				<AcademicPrerequisiteNotice prerequisite={missingGroupsPrerequisite} />
			{/if}
			{#if groupsWithoutTeachers > 0}
				<AcademicPrerequisiteNotice prerequisite={missingTeachersPrerequisite} />
			{/if}
			{#if controller.workspace.bellPeriods.length === 0}
				<AcademicPrerequisiteNotice prerequisite={missingPeriodsPrerequisite} />
			{/if}
			{#if controller.workspace.rooms.length === 0}
				<AcademicPrerequisiteNotice prerequisite={missingRoomsPrerequisite} />
			{/if}

			{#if errorMessage}
				<div
					class="flex items-start gap-2 rounded-xl border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
				>
					<AlertTriangle class="mt-0.5 size-4 shrink-0" />
					<span>{errorMessage}</span>
				</div>
			{/if}

			{#if controller.dragSource}
				<div
					class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-primary/25 bg-primary/5 px-4 py-3"
				>
					<p class="flex items-center gap-2 text-sm font-medium text-primary">
						<MousePointer2 class="size-4" /> เลือกช่องสีเขียวเพื่อวาง 1 คาบ หรือกด Esc เพื่อยกเลิก
					</p>
					<Button variant="ghost" size="sm" onclick={cancelDrag}>ยกเลิก</Button>
				</div>
			{/if}

			{#if controller.selectedRow && controller.workspace.bellPeriods.length > 0}
				<div class="grid min-h-0 gap-4 xl:grid-cols-[19rem_minmax(0,1fr)]">
					<TimetableUnscheduledTray
						demands={visibleDemands}
						groups={controller.workspace.learningGroups}
						staff={controller.workspace.staff}
						disabled={!canEditSelected}
						remainingForGroup={controller.remainingDemand}
						onChooseDemand={chooseDemand}
						onDragStartDemand={(source, candidate) => controller?.startPlacement(source, candidate)}
						onCancelDrag={cancelDrag}
					/>
					<TimetableBoard
						state={controller.board}
						view={controller.view}
						row={controller.selectedRow}
						selectedEntryId={inspectorEntryId}
						canEdit={canEditSelected}
						{cellState}
						onDropIntent={attemptPlacement}
						onActivateIntent={attemptPlacement}
						onSelectEntry={inspectEntry}
						onDragStart={(entry) => startExistingPlacement(entry)}
						onCancelDrag={cancelDrag}
						onMoveEntry={openMoveDialog}
						onEditEntry={inspectEntry}
						onRemoveEntry={requestRemove}
					/>
				</div>
			{:else}
				<PageState
					variant="empty"
					title="ยังไม่มีข้อมูลสำหรับสร้างตาราง"
					description="ตรวจกลุ่มเรียน ห้องประจำชั้น และคาบเรียนของภาคเรียนนี้"
				/>
			{/if}
		</div>
	{/if}

	<Dialog.Root bind:open={teacherDialogOpen}>
		<Dialog.Content class="sm:max-w-lg">
			<Dialog.Header>
				<Dialog.Title>เลือกครูของคาบนี้</Dialog.Title>
				<Dialog.Description>
					กลุ่มนี้มีครูมากกว่าหนึ่งคน เลือกเฉพาะผู้ที่สอนคาบที่จะนำไปวาง
				</Dialog.Description>
			</Dialog.Header>
			<TimetableInstructorPicker
				options={pendingDemandInstructorOptions}
				bind:value={pendingDemandInstructorIds}
			/>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (teacherDialogOpen = false)}>ยกเลิก</Button>
				<Button
					disabled={pendingDemandInstructorIds.length === 0}
					onclick={startDemandAfterTeacherSelection}
				>
					เริ่มวาง 1 คาบ
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>

	<TimetableEntryInspector
		bind:open={inspectorOpen}
		entry={selectedEntry}
		rooms={controller?.workspace.rooms ?? []}
		instructorOptions={instructorOptionsForEntry(selectedEntry)}
		readOnly={!canEditSelected}
		{busy}
		onSave={saveEntryDetails}
		onMove={openMoveDialog}
		onRemove={requestRemove}
	/>

	<TimetableMoveDialog
		bind:open={moveDialogOpen}
		entry={moveEntry}
		periods={controller?.workspace.bellPeriods ?? []}
		{busy}
		onConfirm={confirmMove}
	/>

	<AlertDialog.Root bind:open={removeDialogOpen}>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>นำคาบนี้ออกจากตารางหรือไม่</AlertDialog.Title>
				<AlertDialog.Description>
					{removeEntry?.offeringCode ?? removeEntry?.entryType ?? ''} · {removeEntry?.offeringName ??
						removeEntry?.title ??
						''}
					จะกลับไปอยู่ในรายการคาบที่รอจัด และสามารถนำมาวางใหม่ได้
				</AlertDialog.Description>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Cancel disabled={busy}>ยกเลิก</AlertDialog.Cancel>
				<AlertDialog.Action variant="destructive" disabled={busy} onclick={confirmRemove}>
					{#if busy}<LoaderCircle class="size-4 animate-spin" />{/if}
					{busy ? 'กำลังนำออก...' : 'นำออกจากตาราง'}
				</AlertDialog.Action>
			</AlertDialog.Footer>
		</AlertDialog.Content>
	</AlertDialog.Root>
</PageShell>
