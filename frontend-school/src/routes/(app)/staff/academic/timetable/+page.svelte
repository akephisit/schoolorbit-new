<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';

	import { getAcademicContextStore } from '$lib/academic-context/store';
	import {
		blockBelongsToRow,
		blockHomeroomIds,
		blockTeacherIds,
		blocksForTimetableCell,
		localPlacementPreview,
		type TimetablePageView
	} from '$lib/academic/timetable/board-state';
	import {
		createTimetableWorkspaceController,
		type TimetableDragSource,
		type TimetableWorkspaceController
	} from '$lib/academic/timetable/workspace-controller.svelte';
	import { ApiClientError } from '$lib/api/client';
	import { getAcademicTermChangeSet, type AcademicTermChangeSet } from '$lib/api/learning-delivery';
	import {
		createOrdinaryTimetableBlock,
		createStructuralTimetableBlocks,
		createSynchronizedTimetableBlock,
		deleteTimetableBlock,
		deleteTimetableBlockSeries,
		getTimetableBlockWorkspace,
		listTimetableVersions,
		previewTimetableBlockPlacement,
		removeTimetableBlockTarget,
		swapTimetableBlocks,
		updateTimetableBlock,
		type CreateStructuralTimetableBlocksRequest,
		type TimetableBlock,
		type TimetableBlockPlacementCandidate,
		type TimetableBlockPlacementPreview,
		type TimetableBlockPlacementSource,
		type TimetableStructuralKind,
		type TimetableTargetKind,
		type TimetableVersion,
		type TimetableBlockWorkspace
	} from '$lib/api/timetable';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import TimetableBoard from '$lib/components/academic/timetable/TimetableBoard.svelte';
	import type { TimetableCellState } from '$lib/components/academic/timetable/TimetableCell.svelte';
	import TimetableInstructorPicker from '$lib/components/academic/timetable/TimetableInstructorPicker.svelte';
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
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Textarea } from '$lib/components/ui/textarea';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import {
		connectTimetableSocket,
		disconnectTimetableSocket,
		refreshTrigger
	} from '$lib/stores/timetable-socket';
	import {
		AlertTriangle,
		Check,
		FileSpreadsheet,
		History,
		LoaderCircle,
		MousePointer2,
		Plus,
		RefreshCw,
		Trash2
	} from 'lucide-svelte';

	type RemovalMode = 'target' | 'block' | 'series';
	type StructuralForm = {
		kind: TimetableStructuralKind;
		title: string;
		note: string;
		roomId: string;
		allHomerooms: boolean;
		allTeachers: boolean;
		homeroomIds: string[];
		teacherIds: string[];
		slots: string[];
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
		description: 'กำหนดครูในหน้าจัดการเรียนก่อน เพื่อให้ระบบตรวจตารางครูชนได้ถูกต้อง',
		actionLabel: 'ไปกำหนดครู',
		href: '/staff/academic/delivery'
	};
	const missingPeriodsPrerequisite: AcademicPrerequisite = {
		key: 'timetable-periods',
		status: 'missing',
		title: 'ยังไม่มีคาบในตารางเวลา',
		description: 'ตั้งชื่อ เวลาเริ่ม และเวลาสิ้นสุดของแต่ละคาบก่อนจัดตาราง',
		actionLabel: 'ไปตั้งค่าคาบ',
		href: '/staff/academic/core#bell-schedules'
	};
	const missingRoomsPrerequisite: AcademicPrerequisite = {
		key: 'timetable-rooms',
		status: 'warning',
		title: 'ยังไม่มีห้องเรียนให้เลือก',
		description: 'เพิ่มอาคารและห้องเรียนก่อน หากต้องการตรวจการใช้ห้องชนกัน',
		actionLabel: 'ไปจัดการห้อง',
		href: '/staff/facility/buildings'
	};

	const days = [
		{ id: 'MON', label: 'จันทร์' },
		{ id: 'TUE', label: 'อังคาร' },
		{ id: 'WED', label: 'พุธ' },
		{ id: 'THU', label: 'พฤหัสบดี' },
		{ id: 'FRI', label: 'ศุกร์' }
	];
	const noRoomValue = '__none__';
	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const academicYearId = $derived($academicContext.selected.academicYearId);
	const request = new LatestRequest();
	const placementRequest = new LatestRequest();

	let versions = $state<TimetableVersion[]>([]);
	let controller = $state.raw<TimetableWorkspaceController | null>(null);
	let selectedChangeSet = $state.raw<AcademicTermChangeSet | null>(null);
	let loading = $state(false);
	let busy = $state(false);
	let previewing = $state(false);
	let exportingTeacherLoad = $state(false);
	let errorMessage = $state('');
	let draftRevision = $state(0);
	let activeView = $state<TimetablePageView>('homeroom');
	let previewCellKey = $state('');
	let selectedBlockId = $state<string | null>(null);
	let editOpen = $state(false);
	let editTitle = $state('');
	let editNote = $state('');
	let editRoomId = $state(noRoomValue);
	let editInstructorIds = $state<string[]>([]);
	let removeOpen = $state(false);
	let removeMode = $state<RemovalMode>('block');
	let structuralOpen = $state(false);
	let structuralForm = $state<StructuralForm>(newStructuralForm());
	let overviewDay = $state('MON');

	const canRead = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_TIMETABLE_PUBLISH_SCHOOL
		)
	);
	const canManage = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ASSIGNED
		)
	);
	const canEdit = $derived(Boolean(canManage && controller?.canEdit && !busy));
	const selectedVersion = $derived(controller?.workspace.version ?? null);
	let versionSelectValue = $derived(selectedVersion?.id ?? '');
	const selectedOwner = $derived(
		controller?.rows.find((row) => row.id === controller?.selectedOwnerId) ?? null
	);
	const selectedBlock = $derived(
		controller?.workspace.blocks.find((block) => block.id === selectedBlockId) ?? null
	);
	const activeDraftVersion = $derived(
		versions.find((version) => version.status === 'draft' && version.changeSetId) ?? null
	);
	const groupsWithoutTeachers = $derived(
		controller?.workspace.learningGroups.filter((group) => group.eligibleInstructors.length === 0)
			.length ?? 0
	);
	const visibleOrdinaryDemands = $derived.by(() => {
		if (!controller || !controller.selectedOwnerId) return [];
		if (controller.view === 'homeroom') {
			return controller.workspace.ordinaryDemands.filter((demand) =>
				demand.homeroomIds.includes(controller?.selectedOwnerId ?? '')
			);
		}
		if (controller.view === 'teacher') {
			return controller.workspace.ordinaryDemands.filter((demand) =>
				demand.eligibleInstructors.some(
					(teacher) => teacher.teacherId === controller?.selectedOwnerId
				)
			);
		}
		return controller.workspace.ordinaryDemands.filter(
			(demand) => demand.learningGroupId === controller?.selectedOwnerId
		);
	});
	const visibleSynchronizedDemands = $derived.by(() => {
		if (!controller || !controller.selectedOwnerId) return [];
		if (controller.view === 'homeroom') {
			return controller.workspace.synchronizedDemands.filter((demand) =>
				demand.intendedHomeroomIds.includes(controller?.selectedOwnerId ?? '')
			);
		}
		return controller.workspace.synchronizedDemands;
	});

	function newStructuralForm(): StructuralForm {
		return {
			kind: 'flag_ceremony',
			title: 'กิจกรรมหน้าเสาธง',
			note: '',
			roomId: noRoomValue,
			allHomerooms: true,
			allTeachers: false,
			homeroomIds: [],
			teacherIds: [],
			slots: []
		};
	}

	function requestedView(): TimetablePageView {
		const value = page.url.searchParams.get('view');
		return value === 'teacher' || value === 'learning_group' || value === 'wholeSchool'
			? value
			: 'homeroom';
	}

	function selectPreferredVersion(items: TimetableVersion[]): TimetableVersion | null {
		const requested = page.url.searchParams.get('timetableVersionId');
		return (
			items.find((version) => version.id === requested) ??
			items.find((version) => version.status === 'draft') ??
			items.find((version) => version.displayState === 'current') ??
			items[0] ??
			null
		);
	}

	function versionLabel(version: TimetableVersion): string {
		const state =
			version.status === 'draft'
				? 'แบบร่าง'
				: version.status === 'published'
					? 'เผยแพร่'
					: 'ยกเลิก';
		return `${state} · เริ่ม ${version.effectiveFrom}`;
	}

	function initializeController(workspace: TimetableBlockWorkspace): void {
		const next = createTimetableWorkspaceController(workspace);
		activeView = requestedView();
		next.setView(activeView === 'wholeSchool' ? 'homeroom' : activeView);
		const ownerId = page.url.searchParams.get('ownerId');
		if (ownerId && activeView !== 'wholeSchool') next.selectOwner(ownerId);
		controller = next;
	}

	function loadChangeSet(
		version: TimetableVersion,
		signal?: AbortSignal
	): Promise<AcademicTermChangeSet | null> {
		return version.changeSetId
			? getAcademicTermChangeSet(version.changeSetId, { signal })
			: Promise.resolve(null);
	}

	function syncUrl(): void {
		if (!controller) return;
		const next = new URL(page.url);
		next.searchParams.set('timetableVersionId', controller.workspace.version.id);
		next.searchParams.set('view', activeView);
		if (activeView !== 'wholeSchool' && controller.selectedOwnerId) {
			next.searchParams.set('ownerId', controller.selectedOwnerId);
		} else {
			next.searchParams.delete('ownerId');
		}
		window.history.replaceState(window.history.state, '', next);
	}

	async function loadContext(termId: string, yearId: string): Promise<void> {
		const { revision, signal } = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const loadedVersions = await listTimetableVersions(termId, { signal });
			const selected = selectPreferredVersion(loadedVersions);
			if (!selected) {
				if (!request.isCurrent(revision)) return;
				versions = loadedVersions;
				controller = null;
				selectedChangeSet = null;
				return;
			}
			const workspace = await getTimetableBlockWorkspace(
				{ academicYearId: yearId, academicTermId: termId, timetableVersionId: selected.id },
				{ signal }
			);
			const changeSet = await loadChangeSet(workspace.version, signal);
			if (!request.isCurrent(revision)) return;
			versions = loadedVersions;
			initializeController(workspace);
			selectedChangeSet = changeSet;
			draftRevision += 1;
			syncUrl();
		} catch (error) {
			if (!isAbortError(error) && request.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดตารางสอนไม่สำเร็จ';
			}
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function loadVersion(versionId: string, force = false): Promise<void> {
		if (
			!academicTermId ||
			!academicYearId ||
			(!force && versionId === controller?.workspace.version.id)
		)
			return;
		const { revision, signal } = request.begin();
		loading = true;
		try {
			const workspace = await getTimetableBlockWorkspace(
				{ academicYearId, academicTermId, timetableVersionId: versionId },
				{ signal }
			);
			const changeSet = await loadChangeSet(workspace.version, signal);
			if (!request.isCurrent(revision)) return;
			initializeController(workspace);
			selectedChangeSet = changeSet;
			draftRevision += 1;
			syncUrl();
		} catch (error) {
			if (!isAbortError(error) && request.isCurrent(revision)) {
				toast.error(error instanceof Error ? error.message : 'เปลี่ยนรุ่นตารางสอนไม่สำเร็จ');
			}
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function reload(message?: string): Promise<void> {
		if (!controller || !academicTermId || !academicYearId) return;
		controller.setRefreshing(true);
		try {
			const workspace = await getTimetableBlockWorkspace({
				academicYearId,
				academicTermId,
				timetableVersionId: controller.workspace.version.id
			});
			controller.setWorkspace(workspace);
			draftRevision += 1;
			if (message) toast.success(message);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'โหลดข้อมูลล่าสุดไม่สำเร็จ');
		} finally {
			controller?.setRefreshing(false);
		}
	}

	function changeView(view: TimetablePageView): void {
		if (!controller) return;
		activeView = view;
		if (view !== 'wholeSchool') controller.setView(view);
		controller.clearPlacement();
		syncUrl();
	}

	function changeOwner(ownerId: string): void {
		controller?.selectOwner(ownerId);
		syncUrl();
	}

	function candidateForBlock(block: TimetableBlock): TimetableBlockPlacementCandidate {
		const group = block.groups[0];
		const teacherIds = blockTeacherIds(block);
		return {
			blockKind: block.blockKind,
			learningGroupId: block.groups.length === 1 ? (group?.learningGroupId ?? null) : null,
			learningOfferingId: block.learningOfferingId,
			roomId: group?.roomId ?? block.homerooms[0]?.roomId ?? null,
			instructorIds: block.blockKind === 'structural' ? [] : teacherIds,
			homeroomIds: blockHomeroomIds(block),
			teacherIds: block.blockKind === 'structural' ? teacherIds : []
		};
	}

	function startExistingPlacement(block: TimetableBlock): void {
		controller?.startPlacement(
			{ kind: 'existing_block', blockId: block.id, rowVersion: block.rowVersion },
			candidateForBlock(block)
		);
	}

	function chooseDemand(
		source: TimetableBlockPlacementSource,
		candidate: TimetableBlockPlacementCandidate
	): void {
		controller?.startPlacement(source, candidate);
	}

	function cancelPlacement(): void {
		placementRequest.abort();
		previewing = false;
		previewCellKey = '';
		controller?.clearPlacement();
	}

	function cellKey(dayOfWeek: string, periodId: string): string {
		return `${dayOfWeek}:${periodId}`;
	}

	function cellState(dayOfWeek: string, periodId: string): TimetableCellState {
		if (!controller?.dragSource || !controller.selectedOwnerId) return 'neutral';
		const source = controller.dragSource.source;
		if (source.kind === 'existing_block') {
			const block = controller.board.blocksById.get(source.blockId);
			if (block?.dayOfWeek === dayOfWeek && block.bellSchedulePeriodId === periodId)
				return 'dragging';
		}
		if (controller.preview && previewCellKey === cellKey(dayOfWeek, periodId)) {
			return controller.preview.state === 'source' ? 'dragging' : controller.preview.state;
		}
		const local = localPlacementPreview(controller.board, {
			view: controller.view,
			rowId: controller.selectedOwnerId,
			dayOfWeek,
			bellSchedulePeriodId: periodId,
			...controller.dragSource
		});
		return local.state === 'source' ? 'dragging' : local.state;
	}

	function targetBlock(dayOfWeek: string, periodId: string): TimetableBlock | null {
		if (!controller?.selectedOwnerId) return null;
		const sourceId =
			controller.dragSource?.source.kind === 'existing_block'
				? controller.dragSource.source.blockId
				: null;
		return (
			blocksForTimetableCell(controller.board, {
				view: controller.view,
				rowId: controller.selectedOwnerId,
				dayOfWeek,
				bellSchedulePeriodId: periodId
			}).find((block) => block.id !== sourceId) ?? null
		);
	}

	async function fetchPlacementPreview(
		dayOfWeek: string,
		periodId: string
	): Promise<TimetableBlockPlacementPreview | null> {
		if (!controller?.dragSource || !academicTermId) return null;
		const target = targetBlock(dayOfWeek, periodId);
		const requestedCellKey = cellKey(dayOfWeek, periodId);
		const { revision, signal } = placementRequest.begin();
		previewCellKey = requestedCellKey;
		controller.setPreview(null);
		previewing = true;
		try {
			const preview = await previewTimetableBlockPlacement(
				{
					academicTermId,
					timetableVersionId: controller.workspace.version.id,
					targetDayOfWeek: dayOfWeek,
					targetBellSchedulePeriodId: periodId,
					expectedTargetBlockId: target?.id ?? null,
					expectedTargetRowVersion: target?.rowVersion ?? null,
					...controller.dragSource
				},
				{ signal }
			);
			if (!placementRequest.isCurrent(revision) || previewCellKey !== requestedCellKey) return null;
			controller.setPreview(preview);
			return preview;
		} catch (error) {
			if (!isAbortError(error) && !(error instanceof ApiClientError && error.status === 409)) {
				toast.error(error instanceof Error ? error.message : 'ตรวจตำแหน่งวางคาบไม่สำเร็จ');
			}
			return null;
		} finally {
			if (placementRequest.isCurrent(revision)) previewing = false;
		}
	}

	async function applyPlacement(dayOfWeek: string, periodId: string): Promise<void> {
		if (!controller?.dragSource || !academicTermId || busy) return;
		const dragSource: TimetableDragSource = controller.dragSource;
		const targetCellKey = cellKey(dayOfWeek, periodId);
		const preview =
			!previewing && previewCellKey === targetCellKey && controller.preview
				? controller.preview
				: await fetchPlacementPreview(dayOfWeek, periodId);
		if (!preview || preview.state === 'blocked' || preview.state === 'source') return;
		busy = true;
		try {
			if (dragSource.source.kind === 'ordinary_demand') {
				await createOrdinaryTimetableBlock({
					academicTermId,
					timetableVersionId: controller.workspace.version.id,
					learningGroupId: dragSource.source.learningGroupId,
					dayOfWeek,
					bellSchedulePeriodId: periodId,
					roomId: dragSource.candidate.roomId,
					instructorIds: dragSource.candidate.instructorIds ?? [],
					note: null
				});
			} else if (dragSource.source.kind === 'synchronized_offering') {
				await createSynchronizedTimetableBlock({
					academicTermId,
					timetableVersionId: controller.workspace.version.id,
					learningOfferingId: dragSource.source.learningOfferingId,
					intendedHomeroomIds: dragSource.candidate.homeroomIds ?? [],
					dayOfWeek,
					bellSchedulePeriodId: periodId,
					roomId: dragSource.candidate.roomId,
					note: null
				});
			} else if (preview.state === 'swap' && preview.targetBlockId) {
				const other = controller.board.blocksById.get(preview.targetBlockId);
				if (!other) throw new Error('ไม่พบคาบปลายทาง กรุณาโหลดข้อมูลล่าสุด');
				await swapTimetableBlocks({
					timetableVersionId: controller.workspace.version.id,
					blockAId: dragSource.source.blockId,
					blockARowVersion: dragSource.source.rowVersion,
					blockBId: other.id,
					blockBRowVersion: other.rowVersion
				});
			} else {
				await updateTimetableBlock(dragSource.source.blockId, {
					timetableVersionId: controller.workspace.version.id,
					rowVersion: dragSource.source.rowVersion,
					dayOfWeek,
					bellSchedulePeriodId: periodId
				});
			}
			cancelPlacement();
			await reload('บันทึกตำแหน่งคาบแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'วางคาบไม่สำเร็จ');
		} finally {
			busy = false;
		}
	}

	function previewPlacement(dayOfWeek: string, periodId: string): void {
		if (!controller?.dragSource || previewCellKey === cellKey(dayOfWeek, periodId)) return;
		void fetchPlacementPreview(dayOfWeek, periodId);
	}

	async function exportTeacherLoad(): Promise<void> {
		if (!controller || exportingTeacherLoad) return;
		exportingTeacherLoad = true;
		try {
			const { downloadTeacherLoadWorkbook } =
				await import('$lib/utils/timetable-teacher-load-workbook');
			const selectedTerm = $academicContext.options?.terms.find(
				(term) => term.id === academicTermId
			);
			const selectedYear = $academicContext.options?.years.find(
				(year) => year.id === academicYearId
			);
			const teacherCount = await downloadTeacherLoadWorkbook(
				controller.workspace.blocks,
				`สรุปคาบสอนครู-${selectedTerm?.name ?? 'ภาคเรียน'}-${selectedYear?.name ?? 'ปีการศึกษา'}`
			);
			if (teacherCount === 0) toast.error('ไม่พบคาบสอนสำหรับภาคเรียนนี้');
			else toast.success(`ดาวน์โหลดสรุปคาบสอน ${teacherCount} คนแล้ว`);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ส่งออกสรุปคาบสอนไม่สำเร็จ');
		} finally {
			exportingTeacherLoad = false;
		}
	}

	async function handleRevisionCreated(created: AcademicTermChangeSet): Promise<void> {
		if (!academicTermId) return;
		selectedChangeSet = created;
		versions = await listTimetableVersions(academicTermId);
		await loadVersion(created.targetTimetableVersionId, true);
		toast.success('สร้างรุ่นตารางสอนแบบร่างแล้ว');
	}

	async function handleChangeSetChanged(updated: AcademicTermChangeSet): Promise<void> {
		selectedChangeSet = updated;
		if (!academicTermId) return;
		versions = await listTimetableVersions(academicTermId);
		await loadVersion(updated.targetTimetableVersionId, true);
		if (updated.status === 'published') toast.success('เผยแพร่รุ่นตารางสอนใหม่แล้ว');
		if (updated.status === 'cancelled') toast.success('ยกเลิกรุ่นตารางสอนแบบร่างแล้ว');
	}

	function openEditor(block: TimetableBlock): void {
		selectedBlockId = block.id;
		editTitle = block.title ?? '';
		editNote = block.note ?? '';
		editRoomId = block.groups[0]?.roomId ?? block.homerooms[0]?.roomId ?? noRoomValue;
		editInstructorIds = block.groups.flatMap((group) =>
			group.instructors.map((teacher) => teacher.teacherId)
		);
		editOpen = true;
	}

	function instructorOptionsForBlock(block: TimetableBlock | null) {
		if (!controller || !block || block.groups.length !== 1) return [];
		const blockGroup = block.groups[0];
		const learningGroup = controller.workspace.learningGroups.find(
			(item) => item.id === block.groups[0]?.learningGroupId
		);
		const attached = (blockGroup?.instructors ?? []).map((teacher) => ({
			id: teacher.teacherId,
			displayName: teacher.displayName,
			role:
				teacher.role === 'assistant'
					? ('assistant' as const)
					: teacher.role === 'primary'
						? ('primary' as const)
						: ('secondary' as const)
		}));
		const eligible = (learningGroup?.eligibleInstructors ?? []).map((teacher) => ({
			id: teacher.teacherId,
			displayName: teacher.displayName,
			role:
				teacher.role === 'assistant'
					? ('assistant' as const)
					: teacher.role === 'primary'
						? ('primary' as const)
						: ('secondary' as const)
		}));
		return [
			...attached,
			...eligible.filter(
				(option) => !attached.some((attachedOption) => attachedOption.id === option.id)
			)
		];
	}

	async function saveBlock(): Promise<void> {
		if (!controller || !selectedBlock || busy) return;
		busy = true;
		try {
			await updateTimetableBlock(selectedBlock.id, {
				timetableVersionId: controller.workspace.version.id,
				rowVersion: selectedBlock.rowVersion,
				title: editTitle.trim() || null,
				clearTitle: editTitle.trim().length === 0,
				note: editNote.trim() || null,
				clearNote: editNote.trim().length === 0,
				roomId: editRoomId === noRoomValue ? null : editRoomId,
				clearRoom: editRoomId === noRoomValue,
				instructorIds:
					selectedBlock.blockKind !== 'structural' && selectedBlock.groups.length === 1
						? editInstructorIds
						: null
			});
			editOpen = false;
			await reload('แก้รายละเอียดคาบแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'แก้รายละเอียดคาบไม่สำเร็จ');
		} finally {
			busy = false;
		}
	}

	function currentRemovalTarget(block: TimetableBlock | null): {
		kind: TimetableTargetKind;
		id: string;
		rowVersion: number;
		label: string;
	} | null {
		if (!block || !controller?.selectedOwnerId) return null;
		const ownerId = controller.selectedOwnerId;
		if (controller.view === 'teacher') {
			const teacher = block.teachers.find((item) => item.teacherId === ownerId);
			return teacher
				? {
						kind: 'teacher',
						id: teacher.id,
						rowVersion: teacher.rowVersion,
						label: teacher.displayName
					}
				: null;
		}
		if (controller.view === 'homeroom') {
			const homeroom = block.homerooms.find((item) => item.homeroomId === ownerId);
			if (homeroom) {
				return {
					kind: 'homeroom',
					id: homeroom.id,
					rowVersion: homeroom.rowVersion,
					label: homeroom.name
				};
			}
			const group = block.groups.find((item) => item.homeroomIds.includes(ownerId));
			return group
				? { kind: 'group', id: group.id, rowVersion: group.rowVersion, label: group.name }
				: null;
		}
		const group = block.groups.find((item) => item.learningGroupId === ownerId);
		return group
			? { kind: 'group', id: group.id, rowVersion: group.rowVersion, label: group.name }
			: null;
	}

	function requestRemove(block: TimetableBlock): void {
		selectedBlockId = block.id;
		removeMode =
			currentRemovalTarget(block) && (block.blockKind !== 'course' || block.groups.length > 1)
				? 'target'
				: 'block';
		removeOpen = true;
	}

	async function confirmRemove(): Promise<void> {
		if (!controller || !selectedBlock || busy) return;
		busy = true;
		try {
			if (removeMode === 'target') {
				const target = currentRemovalTarget(selectedBlock);
				if (!target) throw new Error('ไม่พบห้องหรือครูที่จะนำออกจากคาบ');
				await removeTimetableBlockTarget(selectedBlock.id, {
					timetableVersionId: controller.workspace.version.id,
					blockRowVersion: selectedBlock.rowVersion,
					targetKind: target.kind,
					targetId: target.id,
					targetRowVersion: target.rowVersion
				});
			} else if (removeMode === 'series') {
				if (!selectedBlock.seriesId) throw new Error('คาบนี้ไม่ได้อยู่ในชุดคาบพิเศษ');
				await deleteTimetableBlockSeries(selectedBlock.seriesId, controller.workspace.version.id);
			} else {
				await deleteTimetableBlock(
					selectedBlock.id,
					selectedBlock.rowVersion,
					controller.workspace.version.id
				);
			}
			removeOpen = false;
			selectedBlockId = null;
			await reload('นำรายการออกจากตารางแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'นำรายการออกไม่สำเร็จ');
		} finally {
			busy = false;
		}
	}

	function openStructuralDialog(): void {
		structuralForm = newStructuralForm();
		structuralOpen = true;
	}

	function setStructuralKind(kind: TimetableStructuralKind): void {
		const titles: Record<TimetableStructuralKind, string> = {
			break: 'พัก',
			homeroom: 'โฮมรูม',
			flag_ceremony: 'กิจกรรมหน้าเสาธง',
			teacher_meeting: 'ประชุมครู',
			academic: 'กิจกรรมวิชาการ',
			other: 'กิจกรรมอื่น'
		};
		structuralForm.kind = kind;
		structuralForm.title = titles[kind];
		if (kind === 'teacher_meeting') {
			structuralForm.allTeachers = true;
			structuralForm.allHomerooms = false;
		}
	}

	function toggleListValue(key: 'homeroomIds' | 'teacherIds' | 'slots', value: string): void {
		const current = structuralForm[key];
		structuralForm[key] = current.includes(value)
			? current.filter((item) => item !== value)
			: [...current, value];
	}

	async function createStructural(): Promise<void> {
		if (!controller || !academicTermId || busy) return;
		if (!structuralForm.title.trim() || structuralForm.slots.length === 0) {
			toast.error('กรอกชื่อและเลือกอย่างน้อย 1 ช่องเวลา');
			return;
		}
		if (
			!structuralForm.allHomerooms &&
			!structuralForm.allTeachers &&
			structuralForm.homeroomIds.length === 0 &&
			structuralForm.teacherIds.length === 0
		) {
			toast.error('เลือกห้องหรือครูอย่างน้อย 1 รายการ');
			return;
		}
		const body: CreateStructuralTimetableBlocksRequest = {
			academicTermId,
			timetableVersionId: controller.workspace.version.id,
			structuralKind: structuralForm.kind,
			title: structuralForm.title.trim(),
			note: structuralForm.note.trim() || null,
			roomId: structuralForm.roomId === noRoomValue ? null : structuralForm.roomId,
			allHomerooms: structuralForm.allHomerooms,
			allTeachers: structuralForm.allTeachers,
			homeroomIds: structuralForm.homeroomIds,
			teacherIds: structuralForm.teacherIds,
			slots: structuralForm.slots.map((slot) => {
				const [dayOfWeek, bellSchedulePeriodId] = slot.split(':');
				return { dayOfWeek, bellSchedulePeriodId };
			})
		};
		busy = true;
		try {
			await createStructuralTimetableBlocks(body);
			structuralOpen = false;
			await reload('เพิ่มคาบพิเศษแล้ว แต่ละห้องและครูสามารถนำออกแยกกันได้');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'เพิ่มคาบพิเศษไม่สำเร็จ');
		} finally {
			busy = false;
		}
	}

	function periodLabel(period: TimetableBlockWorkspace['bellPeriods'][number]): string {
		return period.name ?? `คาบ ${period.orderIndex}`;
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') cancelPlacement();
	}

	onMount(() => {
		let currentTermId = '';
		let currentYearId = '';
		let userId = '';
		let socketKey = '';
		let loadedContextKey = '';
		const synchronize = () => {
			const key = `${currentYearId}:${currentTermId}`;
			if (currentYearId && currentTermId && key !== loadedContextKey) {
				loadedContextKey = key;
				void loadContext(currentTermId, currentYearId);
			} else if (!currentYearId || !currentTermId) {
				loadedContextKey = '';
				controller = null;
				versions = [];
				selectedChangeSet = null;
			}
			const nextSocketKey = `${currentTermId}:${userId}`;
			if (currentTermId && userId && nextSocketKey !== socketKey) {
				disconnectTimetableSocket();
				connectTimetableSocket({ academicTermId: currentTermId, currentUserId: userId });
				socketKey = nextSocketKey;
			}
		};
		const unsubscribeContext = academicContext.subscribe((state) => {
			const nextTerm = state.selected.academicTermId ?? '';
			const nextYear = state.selected.academicYearId ?? '';
			if (nextTerm === currentTermId && nextYear === currentYearId) return;
			currentTermId = nextTerm;
			currentYearId = nextYear;
			synchronize();
		});
		const unsubscribeAuth = authStore.subscribe((state) => {
			userId = state.user?.id ?? '';
			synchronize();
		});
		const unsubscribeRefresh = refreshTrigger.subscribe((value) => {
			if (value > 0 && controller) void reload();
		});
		return () => {
			request.abort();
			placementRequest.abort();
			unsubscribeContext();
			unsubscribeAuth();
			unsubscribeRefresh();
			disconnectTimetableSocket();
		};
	});
</script>

<svelte:window onkeydown={handleKeydown} />
<MobileDragDropPolyfill />

<PageShell
	title="จัดตารางสอน"
	description="ลากรายวิชาและกิจกรรมลงตาราง ตรวจการชนของห้อง ครู และห้องเรียนก่อนบันทึก"
>
	{#snippet actions()}
		{#if canManage && academicTermId}
			{#if activeDraftVersion}
				<Button
					variant="outline"
					disabled={selectedVersion?.id === activeDraftVersion.id || loading}
					onclick={() => loadVersion(activeDraftVersion.id)}
				>
					<History class="size-4" />
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
			disabled={exportingTeacherLoad || loading || !controller?.workspace.blocks.length}
			onclick={exportTeacherLoad}
		>
			{#if exportingTeacherLoad}
				<LoaderCircle class="size-4 animate-spin" />
			{:else}
				<FileSpreadsheet class="size-4" />
			{/if}
			สรุปคาบ XLSX
		</Button>
		<Button variant="outline" disabled={loading || !controller} onclick={() => reload()}>
			<RefreshCw class={`size-4 ${controller?.isRefreshing ? 'animate-spin' : ''}`} /> โหลดล่าสุด
		</Button>
	{/snippet}

	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูตารางสอน"
			description="ติดต่อผู้ดูแลเพื่อขอสิทธิ์ดูหรือจัดตารางสอน"
		/>
	{:else if !academicTermId || !academicYearId}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาและภาคเรียนก่อน"
			description="ใช้ตัวเลือกบนแถบด้านบนเพื่อกำหนดบริบทของตารางสอน"
		/>
	{:else if loading && !controller}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && !controller}
		<PageState
			variant="error"
			title="โหลดตารางสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadContext(academicTermId, academicYearId)}
		/>
	{:else if versions.length === 0 || !controller}
		<PageState
			variant="empty"
			title="ยังไม่มีรุ่นตารางสอน"
			description="สร้างรุ่นตารางสอนของภาคเรียนนี้จากหน้าจัดการเรียนก่อน"
		/>
	{:else}
		<div class="space-y-4">
			<TimetableWorkspaceHeader
				version={controller.workspace.version}
				view={activeView}
				isSaving={busy || previewing}
				isRefreshing={controller.isRefreshing}
				onViewChange={changeView}
			/>

			<Card.Root class="gap-0 py-0">
				<Card.Content class="grid gap-3 p-3 sm:p-4 lg:grid-cols-2">
					<div class="space-y-1.5">
						<Label class="text-xs text-muted-foreground">รุ่นตารางสอน</Label>
						<Select.Root
							type="single"
							bind:value={versionSelectValue}
							onValueChange={loadVersion}
							disabled={loading || busy}
						>
							<Select.Trigger class="w-full" aria-label="เลือกรุ่นตารางสอน">
								{versionLabel(controller.workspace.version)}
							</Select.Trigger>
							<Select.Content>
								{#each versions as version (version.id)}
									<Select.Item value={version.id}>{versionLabel(version)}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					{#if activeView === 'wholeSchool'}
						<div class="space-y-1.5">
							<Label class="text-xs text-muted-foreground">วันที่ดูภาพรวม</Label>
							<Select.Root type="single" bind:value={overviewDay}>
								<Select.Trigger class="w-full" aria-label="เลือกวันดูภาพรวม">
									{days.find((day) => day.id === overviewDay)?.label ?? 'เลือกวัน'}
								</Select.Trigger>
								<Select.Content>
									{#each days as day (day.id)}
										<Select.Item value={day.id}>วัน{day.label}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
					{:else}
						<div class="space-y-1.5">
							<Label class="text-xs text-muted-foreground">
								{controller.view === 'homeroom'
									? 'ห้องประจำชั้น'
									: controller.view === 'teacher'
										? 'ครูผู้สอน'
										: 'กลุ่มเรียน'}
							</Label>
							<Select.Root
								type="single"
								value={controller.selectedOwnerId ?? ''}
								onValueChange={changeOwner}
								disabled={busy}
							>
								<Select.Trigger class="w-full" aria-label="เลือกรายการสำหรับจัดตาราง">
									{selectedOwner ? `${selectedOwner.code} · ${selectedOwner.label}` : 'เลือกรายการ'}
								</Select.Trigger>
								<Select.Content>
									{#each controller.rows as row (row.id)}
										<Select.Item value={row.id}>{row.code} · {row.label}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
					{/if}
				</Card.Content>
			</Card.Root>

			{#if errorMessage}
				<div
					class="flex items-start gap-2 rounded-xl border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
				>
					<AlertTriangle class="mt-0.5 size-4 shrink-0" />
					{errorMessage}
				</div>
			{/if}
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

			<div class="grid gap-2 sm:grid-cols-3 lg:grid-cols-6">
				<div class="rounded-lg border bg-background px-3 py-2">
					<p class="text-[0.68rem] text-muted-foreground">คาบบนตาราง</p>
					<p class="text-lg font-semibold">{controller.workspace.summary.blockCount}</p>
				</div>
				<div class="rounded-lg border bg-background px-3 py-2">
					<p class="text-[0.68rem] text-muted-foreground">รายวิชารอจัด</p>
					<p class="text-lg font-semibold">{controller.workspace.summary.ordinaryDemandCount}</p>
				</div>
				<div
					class="rounded-lg border border-violet-500/25 bg-violet-50/40 px-3 py-2 dark:bg-violet-950/10"
				>
					<p class="text-[0.68rem] text-muted-foreground">กิจกรรมพร้อมกัน</p>
					<p class="text-lg font-semibold">
						{controller.workspace.summary.synchronizedDemandCount}
					</p>
				</div>
				<div class="rounded-lg border bg-background px-3 py-2">
					<p class="text-[0.68rem] text-muted-foreground">กลุ่มเชื่อมแล้ว</p>
					<p class="text-lg font-semibold">{controller.workspace.summary.linkedGroupCount}</p>
				</div>
				<div class="rounded-lg border bg-background px-3 py-2">
					<p class="text-[0.68rem] text-muted-foreground">รอข้อมูลกลุ่ม</p>
					<p class="text-lg font-semibold">{controller.workspace.summary.waitingGroupCount}</p>
				</div>
				<div class="rounded-lg border border-destructive/25 bg-destructive/5 px-3 py-2">
					<p class="text-[0.68rem] text-muted-foreground">กลุ่มมีปัญหา</p>
					<p class="text-lg font-semibold">{controller.workspace.summary.conflictGroupCount}</p>
				</div>
			</div>

			{#if activeView !== 'wholeSchool' && controller.dragSource}
				<div
					class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-primary/25 bg-primary/5 px-4 py-3"
				>
					<p class="flex items-center gap-2 text-sm font-medium text-primary">
						<MousePointer2 class="size-4" /> ลากหรือแตะช่องสีเขียวเพื่อวาง 1 คาบ ช่องสีแดงมีรายการชน
					</p>
					<Button variant="ghost" size="sm" onclick={cancelPlacement}>ยกเลิก</Button>
				</div>
				{#if controller.preview?.state === 'blocked' && controller.preview.conflicts.length > 0}
					<div
						role="alert"
						class="rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
					>
						<p class="font-medium">ช่องนี้วางไม่ได้</p>
						<ul class="mt-1 list-disc space-y-0.5 pl-5">
							{#each controller.preview.conflicts as conflict (`${conflict.code}:${conflict.targetKind}:${conflict.targetId}:${conflict.existingBlockId}`)}
								<li>{conflict.message}</li>
							{/each}
						</ul>
					</div>
				{/if}
			{/if}

			{#if activeView === 'wholeSchool'}
				<section class="overflow-hidden rounded-xl border bg-background">
					<div class="border-b bg-muted/20 px-4 py-3">
						<h2 class="font-semibold">
							ภาพรวมทั้งโรงเรียน · วัน{days.find((day) => day.id === overviewDay)?.label}
						</h2>
						<p class="text-xs text-muted-foreground">
							มุมมองนี้ใช้ตรวจภาพรวม หากต้องการจัดให้เปิดมุมมองห้องหรือครู
						</p>
					</div>
					<div class="overflow-x-auto">
						<table class="w-full border-collapse text-left text-xs">
							<thead>
								<tr class="bg-muted/35">
									<th class="sticky left-0 z-10 min-w-36 border-b border-r bg-muted/70 px-3 py-2"
										>ห้อง</th
									>
									{#each controller.workspace.bellPeriods as period (period.id)}
										<th class="min-w-44 border-b border-r px-3 py-2 text-center">
											<p class="font-semibold">{periodLabel(period)}</p>
											<p class="font-mono text-[0.65rem] font-normal text-muted-foreground">
												{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
											</p>
										</th>
									{/each}
								</tr>
							</thead>
							<tbody>
								{#each controller.workspace.homerooms as homeroom (homeroom.id)}
									<tr>
										<th class="sticky left-0 z-10 border-b border-r bg-background px-3 py-2"
											>{homeroom.name}</th
										>
										{#each controller.workspace.bellPeriods as period (period.id)}
											{@const blocks = controller.workspace.blocks.filter(
												(block) =>
													block.dayOfWeek === overviewDay &&
													block.bellSchedulePeriodId === period.id &&
													blockBelongsToRow(block, 'homeroom', homeroom.id)
											)}
											<td class="border-b border-r p-1.5 align-top">
												{#each blocks as block (block.id)}
													<button
														type="button"
														class={[
															'mb-1 w-full rounded-md border bg-background p-2 text-left',
															block.blockKind === 'activity' && 'border-l-4 border-l-violet-500',
															block.blockKind === 'structural' && 'border-l-4 border-l-amber-500'
														]}
														onclick={() => openEditor(block)}
													>
														<p class="font-mono text-[0.65rem] font-semibold text-primary">
															{block.offeringCode ?? 'กิจกรรม'}
														</p>
														<p class="line-clamp-2 font-medium">
															{block.offeringName ?? block.title}
														</p>
													</button>
												{/each}
											</td>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				</section>
			{:else if controller.selectedRow && controller.workspace.bellPeriods.length > 0}
				<div class="grid min-h-0 gap-4 xl:grid-cols-[20rem_minmax(0,1fr)]">
					<TimetableUnscheduledTray
						ordinaryDemands={visibleOrdinaryDemands}
						synchronizedDemands={visibleSynchronizedDemands}
						groups={controller.workspace.learningGroups}
						disabled={!canEdit}
						onChooseDemand={chooseDemand}
						onDragStartDemand={(source, candidate) => controller?.startPlacement(source, candidate)}
						onCancelDrag={cancelPlacement}
						onOpenStructural={openStructuralDialog}
					/>
					<TimetableBoard
						state={controller.board}
						view={controller.view}
						row={controller.selectedRow}
						{selectedBlockId}
						{canEdit}
						{cellState}
						onHoverIntent={previewPlacement}
						onDropIntent={applyPlacement}
						onActivateIntent={applyPlacement}
						onSelectBlock={openEditor}
						onDragStart={startExistingPlacement}
						onCancelDrag={cancelPlacement}
						onRemoveBlock={requestRemove}
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

	<Dialog.Root bind:open={editOpen}>
		<Dialog.Content class="sm:max-w-xl">
			<Dialog.Header>
				<Dialog.Title>รายละเอียดคาบ</Dialog.Title>
				<Dialog.Description>
					{selectedBlock?.offeringCode ?? 'คาบพิเศษ'} · {selectedBlock?.offeringName ??
						selectedBlock?.title ??
						''}
				</Dialog.Description>
			</Dialog.Header>
			<div class="space-y-4 py-2">
				<div class="space-y-1.5">
					<Label for="timetable-block-title">ชื่อที่แสดงในตาราง</Label>
					<Input
						id="timetable-block-title"
						bind:value={editTitle}
						disabled={!canEdit}
						placeholder={selectedBlock?.offeringName ?? 'ใช้ชื่อจากรายการเปิดสอน'}
					/>
				</div>
				<div class="space-y-1.5">
					<Label>ห้องเรียน</Label>
					<Select.Root type="single" bind:value={editRoomId} disabled={!canEdit}>
						<Select.Trigger class="w-full" aria-label="เลือกห้องเรียน">
							{editRoomId === noRoomValue
								? 'ไม่ระบุห้อง'
								: (controller?.workspace.rooms.find((room) => room.id === editRoomId)?.name ??
									'เลือกห้อง')}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value={noRoomValue}>ไม่ระบุห้อง</Select.Item>
							{#each controller?.workspace.rooms ?? [] as room (room.id)}
								<Select.Item value={room.id}
									>{room.code ? `${room.code} · ` : ''}{room.name}</Select.Item
								>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				{#if selectedBlock?.blockKind !== 'structural' && selectedBlock?.groups.length === 1}
					<TimetableInstructorPicker
						options={instructorOptionsForBlock(selectedBlock)}
						bind:value={editInstructorIds}
						disabled={!canEdit}
					/>
				{:else if selectedBlock?.blockKind === 'activity'}
					<div
						class="rounded-lg border border-violet-500/25 bg-violet-50/40 p-3 text-xs text-muted-foreground dark:bg-violet-950/10"
					>
						กิจกรรมพร้อมกันจะดึงกลุ่มและครูจากหน้าจัดการเรียน หากเพิ่มกลุ่มภายหลัง
						ระบบจะซิงค์เข้าช่วงกิจกรรมนี้
					</div>
				{/if}
				<div class="space-y-1.5">
					<Label for="timetable-block-note">หมายเหตุ</Label>
					<Textarea id="timetable-block-note" bind:value={editNote} disabled={!canEdit} rows={3} />
				</div>
			</div>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (editOpen = false)}>ปิด</Button>
				{#if canEdit}
					<Button disabled={busy} onclick={saveBlock}>
						{#if busy}<LoaderCircle class="size-4 animate-spin" />{/if} บันทึก
					</Button>
				{/if}
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>

	<Dialog.Root bind:open={structuralOpen}>
		<Dialog.Content class="max-h-[92vh] overflow-y-auto sm:max-w-3xl">
			<Dialog.Header>
				<Dialog.Title>เพิ่มคาบพิเศษ</Dialog.Title>
				<Dialog.Description>
					สร้างพร้อมกันได้หลายห้อง หลายครู และหลายวัน แต่ภายหลังสามารถนำแต่ละห้องหรือครูออกแยกกันได้
				</Dialog.Description>
			</Dialog.Header>
			<div class="grid gap-5 py-2 lg:grid-cols-2">
				<div class="space-y-4">
					<div class="space-y-1.5">
						<Label>ประเภท</Label>
						<Select.Root
							type="single"
							value={structuralForm.kind}
							onValueChange={(value) => setStructuralKind(value as TimetableStructuralKind)}
						>
							<Select.Trigger class="w-full" aria-label="เลือกประเภทคาบพิเศษ">
								{structuralForm.title}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="flag_ceremony">กิจกรรมหน้าเสาธง</Select.Item>
								<Select.Item value="homeroom">โฮมรูม</Select.Item>
								<Select.Item value="break">พัก</Select.Item>
								<Select.Item value="teacher_meeting">ประชุมครู</Select.Item>
								<Select.Item value="academic">กิจกรรมวิชาการ</Select.Item>
								<Select.Item value="other">กิจกรรมอื่น</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-1.5">
						<Label for="structural-title">ชื่อที่แสดง</Label>
						<Input id="structural-title" bind:value={structuralForm.title} />
					</div>
					<div class="space-y-1.5">
						<Label>ห้องที่ใช้</Label>
						<Select.Root type="single" bind:value={structuralForm.roomId}>
							<Select.Trigger class="w-full" aria-label="เลือกห้องสำหรับคาบพิเศษ">
								{structuralForm.roomId === noRoomValue
									? 'ไม่ระบุห้อง'
									: (controller?.workspace.rooms.find((room) => room.id === structuralForm.roomId)
											?.name ?? 'เลือกห้อง')}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value={noRoomValue}>ไม่ระบุห้อง</Select.Item>
								{#each controller?.workspace.rooms ?? [] as room (room.id)}
									<Select.Item value={room.id}
										>{room.code ? `${room.code} · ` : ''}{room.name}</Select.Item
									>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-1.5">
						<Label for="structural-note">หมายเหตุ</Label>
						<Textarea id="structural-note" bind:value={structuralForm.note} rows={3} />
					</div>
				</div>

				<div class="space-y-5">
					<section class="space-y-2">
						<div class="flex items-center justify-between gap-2">
							<Label>ห้องประจำชั้น</Label>
							<Button
								type="button"
								size="sm"
								variant={structuralForm.allHomerooms ? 'default' : 'outline'}
								onclick={() => (structuralForm.allHomerooms = !structuralForm.allHomerooms)}
							>
								{#if structuralForm.allHomerooms}<Check class="size-3.5" />{/if} ทุกห้อง
							</Button>
						</div>
						{#if !structuralForm.allHomerooms}
							<div class="flex max-h-32 flex-wrap gap-1.5 overflow-y-auto rounded-lg border p-2">
								{#each controller?.workspace.homerooms ?? [] as homeroom (homeroom.id)}
									<Button
										type="button"
										size="sm"
										variant={structuralForm.homeroomIds.includes(homeroom.id)
											? 'default'
											: 'outline'}
										onclick={() => toggleListValue('homeroomIds', homeroom.id)}
									>
										{homeroom.name}
									</Button>
								{/each}
							</div>
						{/if}
					</section>
					<section class="space-y-2">
						<div class="flex items-center justify-between gap-2">
							<Label>ครู</Label>
							<Button
								type="button"
								size="sm"
								variant={structuralForm.allTeachers ? 'default' : 'outline'}
								onclick={() => (structuralForm.allTeachers = !structuralForm.allTeachers)}
							>
								{#if structuralForm.allTeachers}<Check class="size-3.5" />{/if} ครูทุกคน
							</Button>
						</div>
						{#if !structuralForm.allTeachers}
							<div class="flex max-h-32 flex-wrap gap-1.5 overflow-y-auto rounded-lg border p-2">
								{#each controller?.workspace.staff ?? [] as teacher (teacher.id)}
									<Button
										type="button"
										size="sm"
										variant={structuralForm.teacherIds.includes(teacher.id) ? 'default' : 'outline'}
										onclick={() => toggleListValue('teacherIds', teacher.id)}
									>
										{teacher.displayName}
									</Button>
								{/each}
							</div>
						{/if}
					</section>
				</div>
			</div>

			<section class="space-y-2 border-t pt-4">
				<div class="flex items-center justify-between gap-2">
					<div>
						<Label>ช่องเวลาที่ต้องการเพิ่ม</Label>
						<p class="text-xs text-muted-foreground">เลือกได้หลายวันและหลายคาบในครั้งเดียว</p>
					</div>
					<Badge variant="secondary">{structuralForm.slots.length} ช่อง</Badge>
				</div>
				<div class="overflow-x-auto rounded-lg border">
					<table class="w-full border-collapse text-xs">
						<thead>
							<tr class="bg-muted/35">
								<th class="min-w-24 border-b border-r p-2 text-left">วัน / คาบ</th>
								{#each controller?.workspace.bellPeriods ?? [] as period (period.id)}
									<th class="min-w-28 border-b border-r p-2 text-center">{periodLabel(period)}</th>
								{/each}
							</tr>
						</thead>
						<tbody>
							{#each days as day (day.id)}
								<tr>
									<th class="border-b border-r p-2 text-left">{day.label}</th>
									{#each controller?.workspace.bellPeriods ?? [] as period (period.id)}
										{@const slot = `${day.id}:${period.id}`}
										<td class="border-b border-r p-1">
											<Button
												type="button"
												size="sm"
												variant={structuralForm.slots.includes(slot) ? 'default' : 'ghost'}
												class="w-full"
												aria-pressed={structuralForm.slots.includes(slot)}
												onclick={() => toggleListValue('slots', slot)}
											>
												{#if structuralForm.slots.includes(slot)}<Check class="size-3.5" /> เลือกแล้ว{:else}เลือก{/if}
											</Button>
										</td>
									{/each}
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</section>
			<Dialog.Footer>
				<Button variant="outline" disabled={busy} onclick={() => (structuralOpen = false)}
					>ยกเลิก</Button
				>
				<Button disabled={busy} onclick={createStructural}>
					{#if busy}<LoaderCircle class="size-4 animate-spin" />{:else}<Plus class="size-4" />{/if}
					เพิ่ม {structuralForm.slots.length} ช่อง
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>

	<AlertDialog.Root bind:open={removeOpen}>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>นำรายการออกจากตาราง</AlertDialog.Title>
				<AlertDialog.Description>
					เลือกขอบเขตที่ต้องการลบ ระบบจะไม่ลบรายวิชาหรือกลุ่มต้นทางจากหน้าจัดการเรียน
				</AlertDialog.Description>
			</AlertDialog.Header>
			<div class="space-y-2 py-2">
				{#if currentRemovalTarget(selectedBlock) && (selectedBlock?.blockKind !== 'course' || (selectedBlock?.groups.length ?? 0) > 1)}
					<button
						type="button"
						class={[
							'w-full rounded-lg border p-3 text-left',
							removeMode === 'target' && 'border-primary bg-primary/5 ring-1 ring-primary'
						]}
						onclick={() => (removeMode = 'target')}
					>
						<p class="text-sm font-medium">
							นำออกเฉพาะ {currentRemovalTarget(selectedBlock)?.label}
						</p>
						<p class="text-xs text-muted-foreground">
							ห้องหรือครูอื่นที่เพิ่มมาพร้อมกันยังอยู่ตามเดิม
						</p>
					</button>
				{/if}
				<button
					type="button"
					class={[
						'w-full rounded-lg border p-3 text-left',
						removeMode === 'block' && 'border-primary bg-primary/5 ring-1 ring-primary'
					]}
					onclick={() => (removeMode = 'block')}
				>
					<p class="text-sm font-medium">ลบคาบนี้ทั้งช่อง</p>
					<p class="text-xs text-muted-foreground">
						ลบเฉพาะวันและคาบที่เลือก รายการช่องอื่นไม่เปลี่ยน
					</p>
				</button>
				{#if selectedBlock?.seriesId}
					<button
						type="button"
						class={[
							'w-full rounded-lg border p-3 text-left',
							removeMode === 'series' &&
								'border-destructive bg-destructive/5 ring-1 ring-destructive'
						]}
						onclick={() => (removeMode = 'series')}
					>
						<p class="text-sm font-medium text-destructive">ลบทั้งชุดที่สร้างพร้อมกัน</p>
						<p class="text-xs text-muted-foreground">ลบทุกวันและทุกคาบในชุดนี้</p>
					</button>
				{/if}
			</div>
			<AlertDialog.Footer>
				<AlertDialog.Cancel disabled={busy}>ยกเลิก</AlertDialog.Cancel>
				<AlertDialog.Action variant="destructive" disabled={busy} onclick={confirmRemove}>
					{#if busy}<LoaderCircle class="size-4 animate-spin" />{:else}<Trash2
							class="size-4"
						/>{/if}
					ยืนยันนำออก
				</AlertDialog.Action>
			</AlertDialog.Footer>
		</AlertDialog.Content>
	</AlertDialog.Root>
</PageShell>
