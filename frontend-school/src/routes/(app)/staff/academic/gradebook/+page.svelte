<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import {
		selectNewGradebookItem,
		type GradebookCellPosition,
		type ScorePasteMutation
	} from '$lib/academic/gradebook/ledger';
	import {
		createGradebookSaveQueue,
		type GradebookSaveQueue,
		type GradebookSaveQueueSnapshot
	} from '$lib/academic/gradebook/save-queue';
	import {
		confirmGradebookPhase,
		createGradebookItem,
		getGradebookGroupPhaseWorkspace,
		listGradebookControls,
		listGradebookSubjects,
		removeGradebookItem,
		saveGradebookScoresBatch,
		updateGradebookControl,
		updateGradebookItem,
		type GradebookControl,
		type GradebookItemInput,
		type GradebookPhaseCode,
		type GradebookScoreBatchInput,
		type GradebookScoreItem,
		type GradebookSubject,
		type GroupPhaseWorkspace
	} from '$lib/api/academicGradebook';
	import {
		confirmLearnerEvaluationGroup,
		createSubjectEvaluationCriterion,
		getLearnerEvaluationConfiguration,
		getLearnerEvaluationWorkspace,
		listLearnerEvaluationControls,
		listLearnerEvaluationSubjects,
		removeSubjectEvaluationCriterion,
		saveLearnerEvaluationResponses,
		updateLearnerEvaluationControl,
		updateSubjectEvaluationCriterion,
		type LearnerEvaluationConfiguration,
		type LearnerEvaluationControl,
		type LearnerEvaluationCriterion,
		type LearnerEvaluationDomain,
		type LearnerEvaluationResponseBatchInput,
		type LearnerEvaluationSubject,
		type LearnerEvaluationWorkspace
	} from '$lib/api/academicLearnerEvaluations';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import AcademicPrerequisiteNotice from '$lib/components/academic-workflow/AcademicPrerequisiteNotice.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import GradebookEntryControls from '$lib/components/academic/gradebook/GradebookEntryControls.svelte';
	import GradebookMobileEditor from '$lib/components/academic/gradebook/GradebookMobileEditor.svelte';
	import GradebookWorkspaceHeader, {
		type GradebookWorkspaceSubject
	} from '$lib/components/academic/gradebook/GradebookWorkspaceHeader.svelte';
	import LearnerEvaluationLedger from '$lib/components/academic/gradebook/LearnerEvaluationLedger.svelte';
	import PhaseConfirmationDialog from '$lib/components/academic/gradebook/PhaseConfirmationDialog.svelte';
	import ScoreItemDialog from '$lib/components/academic/gradebook/ScoreItemDialog.svelte';
	import ScoreLedger from '$lib/components/academic/gradebook/ScoreLedger.svelte';
	import SubjectCriteriaDialog from '$lib/components/academic/gradebook/SubjectCriteriaDialog.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Tabs from '$lib/components/ui/tabs';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		AlertCircle,
		BookOpenCheck,
		Check,
		ClipboardCheck,
		Cloud,
		Loader2,
		RotateCcw,
		ShieldCheck,
		Trash2
	} from 'lucide-svelte';

	type WorkspaceTab = 'scores' | LearnerEvaluationDomain;
	type ScoreMutation = GradebookScoreBatchInput['cells'][number];
	type EvaluationMutation = LearnerEvaluationResponseBatchInput['cells'][number];
	type MobileCell =
		| { mode: 'score'; itemId: string; studentId: string }
		| { mode: 'evaluation'; criterionId: string; studentId: string };

	const academicContext = getAcademicContextStore();
	const subjectsRequest = new LatestRequest();
	const workspaceRequest = new LatestRequest();
	const criteriaRequest = new LatestRequest();
	const phaseCodes: GradebookPhaseCode[] = ['before_midterm', 'midterm', 'after_midterm', 'final'];
	const phaseLabels: Record<GradebookPhaseCode, string> = {
		before_midterm: 'ก่อนกลางภาค',
		midterm: 'กลางภาค',
		after_midterm: 'หลังกลางภาค',
		final: 'ปลายภาค'
	};
	const tabLabels: Record<WorkspaceTab, string> = {
		scores: 'คะแนนรายวิชา',
		desirable_characteristic: 'คุณลักษณะอันพึงประสงค์',
		reading_thinking_writing: 'การอ่าน คิดวิเคราะห์ และเขียน'
	};
	const evaluationOptions = [
		{ value: '3', label: '3 · ดีเยี่ยม' },
		{ value: '2', label: '2 · ดี' },
		{ value: '1', label: '1 · ผ่าน' },
		{ value: '0', label: '0 · ไม่ผ่าน' },
		{ value: 'blank', label: 'ยังไม่ประเมิน' }
	];
	const savedSnapshot: GradebookSaveQueueSnapshot = {
		state: 'saved',
		pendingCount: 0,
		error: null
	};

	let scoreSubjects = $state.raw<GradebookSubject[]>([]);
	let evaluationSubjects = $state.raw<LearnerEvaluationSubject[]>([]);
	let gradebookControls = $state.raw<GradebookControl[]>([]);
	let evaluationControls = $state.raw<LearnerEvaluationControl[]>([]);
	let scoreWorkspace = $state.raw<GroupPhaseWorkspace | null>(null);
	let evaluationWorkspace = $state.raw<LearnerEvaluationWorkspace | null>(null);
	let evaluationConfiguration = $state.raw<LearnerEvaluationConfiguration | null>(null);
	let scoreValues = $state.raw<Record<string, string | null>>({});
	let scoreVersions = $state.raw<Record<string, number | null>>({});
	let evaluationValues = $state.raw<Record<string, number | null>>({});
	let evaluationVersions = $state.raw<Record<string, number | null>>({});
	let selectedItemIds = $state.raw<string[]>([]);
	let selectedCriterionIds = $state.raw<string[]>([]);
	let selectedSubjectId = $state(page.url.searchParams.get('subjectId')?.trim() ?? '');
	let selectedGroupId = $state(page.url.searchParams.get('learningGroupId')?.trim() ?? '');
	let activeTab = $state<WorkspaceTab>(tabFromUrl());
	let activePhase = $state<GradebookPhaseCode>(phaseFromUrl());
	let loading = $state(false);
	let workspaceLoading = $state(false);
	let errorMessage = $state('');
	let workspaceError = $state('');
	let controlBusyKey = $state('');
	let itemBusy = $state(false);
	let criteriaBusy = $state(false);
	let confirming = $state(false);
	let itemDialogOpen = $state(false);
	let itemDialogRevision = $state(0);
	let editingItem = $state.raw<GradebookScoreItem | null>(null);
	let criteriaDialogOpen = $state(false);
	let confirmationDialogOpen = $state(false);
	let mobileCell = $state.raw<MobileCell | null>(null);
	let scoreSaveStatus = $state.raw<GradebookSaveQueueSnapshot>(savedSnapshot);
	let evaluationSaveStatus = $state.raw<GradebookSaveQueueSnapshot>(savedSnapshot);

	const academicYearId = $derived($academicContext.selected.academicYearId);
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const canReadScores = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_GRADEBOOK_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_GRADEBOOK_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_GRADEBOOK_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_GRADEBOOK_MANAGE_SCHOOL
		)
	);
	const canReadEvaluations = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL
		)
	);
	const canManageGradebookSchool = $derived($can.has(PERMISSIONS.ACADEMIC_GRADEBOOK_MANAGE_SCHOOL));
	const canManageEvaluationSchool = $derived(
		$can.has(PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL)
	);
	const activeSubjects = $derived.by(() => workspaceSubjects(rowsForTab(activeTab)));
	const selectedScoreSubject = $derived(
		scoreSubjects.find((subject) => subject.learningGroupId === selectedGroupId) ?? null
	);
	const selectedPhaseAvailable = $derived(
		selectedScoreSubject?.phases.some((phase) => phase.phaseCode === activePhase) ?? false
	);
	const activeSaveStatus = $derived(
		activeTab === 'scores' ? scoreSaveStatus : evaluationSaveStatus
	);
	const scoreBlankCount = $derived.by(() => {
		if (!scoreWorkspace) return 0;
		const activeItems = scoreWorkspace.items.filter((item) => item.lifecycle === 'active');
		let count = 0;
		for (const student of scoreWorkspace.students) {
			for (const item of activeItems) {
				if (scoreValues[cellKey(student.studentAcademicYearId, item.id)] == null) count += 1;
			}
		}
		return count;
	});
	const nextItemDisplayOrder = $derived(
		Math.max(0, ...(scoreWorkspace?.items.map((item) => item.displayOrder) ?? [])) + 1
	);
	const mobileContext = $derived.by(() => {
		if (!mobileCell) return null;
		if (mobileCell.mode === 'score' && scoreWorkspace) {
			const student = scoreWorkspace.students.find(
				(row) => row.studentAcademicYearId === mobileCell?.studentId
			);
			const item = scoreWorkspace.items.find(
				(row) => mobileCell?.mode === 'score' && row.id === mobileCell.itemId
			);
			if (!student || !item) return null;
			return {
				mode: 'score' as const,
				studentName: student.displayName,
				itemName: item.name,
				maxScore: item.maxScore,
				value: scoreValues[cellKey(student.studentAcademicYearId, item.id)] ?? ''
			};
		}
		if (mobileCell.mode === 'evaluation' && evaluationWorkspace) {
			const student = evaluationWorkspace.students.find(
				(row) => row.studentAcademicYearId === mobileCell?.studentId
			);
			const criterion = evaluationWorkspace.criteria.find(
				(row) => mobileCell?.mode === 'evaluation' && row.id === mobileCell.criterionId
			);
			if (!student || !criterion) return null;
			return {
				mode: 'evaluation' as const,
				studentName: student.displayName,
				itemName: criterion.name,
				maxScore: null,
				value: String(
					evaluationValues[cellKey(student.studentAcademicYearId, criterion.id)] ?? 'blank'
				)
			};
		}
		return null;
	});

	let scoreQueue: GradebookSaveQueue<ScoreMutation>;
	let evaluationQueue: GradebookSaveQueue<EvaluationMutation>;

	function tabFromUrl(): WorkspaceTab {
		const value = page.url.searchParams.get('tab');
		return value === 'desirable_characteristic' || value === 'reading_thinking_writing'
			? value
			: 'scores';
	}

	function phaseFromUrl(): GradebookPhaseCode {
		const value = page.url.searchParams.get('phase');
		return phaseCodes.includes(value as GradebookPhaseCode)
			? (value as GradebookPhaseCode)
			: 'before_midterm';
	}

	function cellKey(studentId: string, itemId: string): string {
		return `${studentId}:${itemId}`;
	}

	function rowsForTab(tab: WorkspaceTab): Array<GradebookSubject | LearnerEvaluationSubject> {
		return tab === 'scores' ? scoreSubjects : evaluationSubjects;
	}

	function workspaceSubjects(
		rows: Array<GradebookSubject | LearnerEvaluationSubject>
	): GradebookWorkspaceSubject[] {
		return rows
			.map((row) => ({
				subjectId: row.subjectId,
				learningGroupId: row.learningGroupId,
				code: row.code,
				name: row.name,
				groupName: row.groupName,
				assigned: row.assigned
			}))
			.toSorted((left, right) => {
				if (left.assigned !== right.assigned) return left.assigned ? -1 : 1;
				return `${left.code}:${left.groupName}`.localeCompare(
					`${right.code}:${right.groupName}`,
					'th-TH',
					{ numeric: true }
				);
			});
	}

	function contextValue(): { academicYearId: string; academicTermId: string } | null {
		if (!academicYearId || !academicTermId) return null;
		return { academicYearId, academicTermId };
	}

	function syncUrl(): void {
		const url = new URL(page.url);
		if (selectedSubjectId) url.searchParams.set('subjectId', selectedSubjectId);
		else url.searchParams.delete('subjectId');
		if (selectedGroupId) url.searchParams.set('learningGroupId', selectedGroupId);
		else url.searchParams.delete('learningGroupId');
		url.searchParams.set('tab', activeTab);
		url.searchParams.set('phase', activePhase);
		void goto(resolve(`/staff/academic/gradebook?${url.searchParams.toString()}`), {
			replaceState: true,
			noScroll: true,
			keepFocus: true
		});
	}

	function hydrateScoreWorkspace(workspace: GroupPhaseWorkspace): void {
		const values: Record<string, string | null> = {};
		const versions: Record<string, number | null> = {};
		for (const score of workspace.scores) {
			const key = cellKey(score.studentAcademicYearId, score.scoreItemId);
			values[key] = score.value ?? null;
			versions[key] = score.rowVersion ?? null;
		}
		scoreValues = values;
		scoreVersions = versions;
		selectedItemIds = selectedItemIds.filter((id) =>
			workspace.items.some((item) => item.id === id && item.lifecycle === 'active')
		);
	}

	function hydrateEvaluationWorkspace(workspace: LearnerEvaluationWorkspace): void {
		const values: Record<string, number | null> = {};
		const versions: Record<string, number | null> = {};
		for (const response of workspace.responses) {
			const key = cellKey(response.studentAcademicYearId, response.subjectTermCriterionId);
			values[key] = response.qualityLevel;
			versions[key] = response.rowVersion;
		}
		evaluationValues = values;
		evaluationVersions = versions;
		selectedCriterionIds = selectedCriterionIds.filter((id) =>
			workspace.criteria.some(
				(criterion) => criterion.id === id && criterion.lifecycle === 'active'
			)
		);
	}

	function scoreMutationValue(mutation: ScoreMutation): string | null {
		return mutation.operation === 'set' ? mutation.value : null;
	}

	function patchScoreCells(cells: GroupPhaseWorkspace['scores']): void {
		if (!scoreWorkspace) return;
		const changedKeys = cells.map((cell) => cellKey(cell.studentAcademicYearId, cell.scoreItemId));
		const next = scoreWorkspace.scores.filter(
			(cell) => !changedKeys.includes(cellKey(cell.studentAcademicYearId, cell.scoreItemId))
		);
		for (const cell of cells) {
			if (cell.value != null) next.push(cell);
		}
		scoreWorkspace = {
			...scoreWorkspace,
			scores: next,
			confirmationIsCurrent: false
		};
	}

	scoreQueue = createGradebookSaveQueue<ScoreMutation>({
		keyOf: (mutation) => cellKey(mutation.studentAcademicYearId, mutation.scoreItemId),
		saveBatch: async (mutations) => {
			const context = contextValue();
			const groupId = selectedGroupId;
			const phaseCode = activePhase;
			if (!context || !groupId) throw new Error('กรุณาเลือกรายวิชาและกลุ่มเรียนก่อน');
			const result = await saveGradebookScoresBatch(groupId, phaseCode, context, {
				cells: mutations
			});
			if (groupId !== selectedGroupId || phaseCode !== activePhase || activeTab !== 'scores')
				return;
			const nextVersions = { ...scoreVersions };
			for (const cell of result.cells) {
				const key = cellKey(cell.studentAcademicYearId, cell.scoreItemId);
				nextVersions[key] = cell.rowVersion ?? null;
				const sent = mutations.find(
					(mutation) =>
						mutation.scoreItemId === cell.scoreItemId &&
						mutation.studentAcademicYearId === cell.studentAcademicYearId
				);
				const currentValue = scoreValues[key] ?? null;
				if (sent && currentValue !== scoreMutationValue(sent)) {
					scoreQueue.enqueue(
						currentValue === null
							? {
									operation: 'clear',
									scoreItemId: cell.scoreItemId,
									studentAcademicYearId: cell.studentAcademicYearId,
									rowVersion: cell.rowVersion ?? null
								}
							: {
									operation: 'set',
									scoreItemId: cell.scoreItemId,
									studentAcademicYearId: cell.studentAcademicYearId,
									value: currentValue,
									rowVersion: cell.rowVersion ?? null
								}
					);
				}
			}
			scoreVersions = nextVersions;
			patchScoreCells(result.cells);
			if (scoreWorkspace) {
				scoreWorkspace = { ...scoreWorkspace, sourceChecksum: result.workspaceRevision };
			}
		}
	});

	evaluationQueue = createGradebookSaveQueue<EvaluationMutation>({
		keyOf: (mutation) => cellKey(mutation.studentAcademicYearId, mutation.subjectTermCriterionId),
		saveBatch: async (mutations) => {
			const context = contextValue();
			const groupId = selectedGroupId;
			const domain = activeTab;
			if (!context || !groupId || domain === 'scores') {
				throw new Error('กรุณาเลือกรายวิชาและด้านประเมินก่อน');
			}
			const result = await saveLearnerEvaluationResponses(groupId, domain, context, {
				cells: mutations
			});
			if (groupId !== selectedGroupId || activeTab !== domain) return;
			const nextVersions = { ...evaluationVersions };
			for (const response of result.responses) {
				nextVersions[cellKey(response.studentAcademicYearId, response.subjectTermCriterionId)] =
					response.rowVersion;
			}
			for (const sent of mutations) {
				const key = cellKey(sent.studentAcademicYearId, sent.subjectTermCriterionId);
				const currentValue = evaluationValues[key] ?? null;
				if (currentValue !== (sent.qualityLevel ?? null)) {
					evaluationQueue.enqueue({
						subjectTermCriterionId: sent.subjectTermCriterionId,
						studentAcademicYearId: sent.studentAcademicYearId,
						qualityLevel: currentValue,
						rowVersion: nextVersions[key] ?? null
					});
				}
			}
			evaluationVersions = nextVersions;
			evaluationWorkspace = result;
		}
	});

	async function loadManagerControls(context: {
		academicYearId: string;
		academicTermId: string;
	}): Promise<void> {
		if (!canManageGradebookSchool && !canManageEvaluationSchool) return;
		try {
			if (canManageGradebookSchool) gradebookControls = await listGradebookControls(context);
			if (canManageEvaluationSchool) {
				evaluationControls = await listLearnerEvaluationControls(context);
			}
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'โหลดช่วงเวลาการกรอกไม่สำเร็จ');
		}
	}

	async function loadSubjects(context: {
		academicYearId: string;
		academicTermId: string;
	}): Promise<void> {
		const { revision, signal } = subjectsRequest.begin();
		loading = true;
		errorMessage = '';
		try {
			const [scores, evaluations] = await Promise.all([
				canReadScores ? listGradebookSubjects(context, { signal }) : Promise.resolve([]),
				canReadEvaluations
					? listLearnerEvaluationSubjects(context, { signal })
					: Promise.resolve([])
			]);
			if (!subjectsRequest.isCurrent(revision)) return;
			scoreSubjects = scores;
			evaluationSubjects = evaluations;
			if (!canReadScores && canReadEvaluations && activeTab === 'scores') {
				activeTab = 'desirable_characteristic';
			}
			if (!canReadEvaluations && activeTab !== 'scores') activeTab = 'scores';
			await ensureSelection();
		} catch (error) {
			if (isAbortError(error)) return;
			if (subjectsRequest.isCurrent(revision)) {
				errorMessage =
					error instanceof Error ? error.message : 'โหลดรายวิชาสำหรับกรอกข้อมูลไม่สำเร็จ';
			}
		} finally {
			if (subjectsRequest.isCurrent(revision)) loading = false;
		}
	}

	async function ensureSelection(): Promise<void> {
		const rows = workspaceSubjects(rowsForTab(activeTab));
		const selected = rows.find(
			(row) => row.subjectId === selectedSubjectId && row.learningGroupId === selectedGroupId
		);
		const sameSubject = rows.find((row) => row.subjectId === selectedSubjectId);
		const next = selected ?? sameSubject ?? rows[0] ?? null;
		selectedSubjectId = next?.subjectId ?? '';
		selectedGroupId = next?.learningGroupId ?? '';
		syncUrl();
		await loadSelectedWorkspace();
	}

	async function loadSelectedWorkspace(): Promise<void> {
		workspaceRequest.abort();
		workspaceError = '';
		scoreWorkspace = null;
		evaluationWorkspace = null;
		evaluationConfiguration = null;
		if (!selectedGroupId) return;
		const context = contextValue();
		if (!context) return;
		if (activeTab === 'scores' && !selectedPhaseAvailable) return;
		const { revision, signal } = workspaceRequest.begin();
		workspaceLoading = true;
		try {
			if (activeTab === 'scores') {
				const result = await getGradebookGroupPhaseWorkspace(
					selectedGroupId,
					activePhase,
					context,
					{ signal }
				);
				if (!workspaceRequest.isCurrent(revision)) return;
				scoreWorkspace = result;
				hydrateScoreWorkspace(result);
			} else {
				const result = await getLearnerEvaluationWorkspace(selectedGroupId, activeTab, context, {
					signal
				});
				if (!workspaceRequest.isCurrent(revision)) return;
				evaluationWorkspace = result;
				hydrateEvaluationWorkspace(result);
			}
		} catch (error) {
			if (isAbortError(error)) return;
			if (workspaceRequest.isCurrent(revision)) {
				workspaceError = error instanceof Error ? error.message : 'โหลดพื้นที่กรอกข้อมูลไม่สำเร็จ';
			}
		} finally {
			if (workspaceRequest.isCurrent(revision)) workspaceLoading = false;
		}
	}

	async function flushPendingWork(): Promise<boolean> {
		try {
			await Promise.all([scoreQueue.flush(), evaluationQueue.flush()]);
			return true;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกข้อมูลไม่สำเร็จ');
			return false;
		}
	}

	async function retryPendingWork(): Promise<void> {
		try {
			if (scoreSaveStatus.state === 'failed') await scoreQueue.retry();
			if (evaluationSaveStatus.state === 'failed') await evaluationQueue.retry();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกข้อมูลไม่สำเร็จ');
		}
	}

	function discardPendingWork(): void {
		scoreQueue.discard();
		evaluationQueue.discard();
		void loadSelectedWorkspace();
	}

	async function changeSelection(subjectId: string, groupId: string): Promise<void> {
		if (subjectId === selectedSubjectId && groupId === selectedGroupId) return;
		if (!(await flushPendingWork())) return;
		mobileCell = null;
		selectedSubjectId = subjectId;
		selectedGroupId = groupId;
		selectedItemIds = [];
		selectedCriterionIds = [];
		syncUrl();
		await loadSelectedWorkspace();
	}

	async function changeTab(value: string): Promise<void> {
		const next: WorkspaceTab =
			value === 'desirable_characteristic' || value === 'reading_thinking_writing'
				? value
				: 'scores';
		if (next === activeTab) return;
		if (!(await flushPendingWork())) return;
		mobileCell = null;
		activeTab = next;
		selectedItemIds = [];
		selectedCriterionIds = [];
		await ensureSelection();
	}

	async function changePhase(phase: GradebookPhaseCode): Promise<void> {
		if (phase === activePhase) return;
		if (!(await flushPendingWork())) return;
		mobileCell = null;
		activePhase = phase;
		selectedItemIds = [];
		syncUrl();
		await loadSelectedWorkspace();
	}

	function applyScoreMutations(mutations: ScorePasteMutation[]): boolean {
		if (!scoreWorkspace?.canManage) return false;
		for (const mutation of mutations) {
			const item = scoreWorkspace.items.find((candidate) => candidate.id === mutation.itemId);
			if (!item || item.lifecycle !== 'active') {
				toast.error('รายการคะแนนนี้ไม่ได้ใช้งานแล้ว');
				return false;
			}
			if (mutation.value !== null && !/^(0|[1-9]\d*)(\.\d{1,2})?$/.test(mutation.value)) {
				toast.error(`${item.name} ต้องเป็นเลขตั้งแต่ 0 และมีทศนิยมไม่เกิน 2 ตำแหน่ง`);
				return false;
			}
			if (mutation.value !== null && Number(mutation.value) > Number(item.maxScore)) {
				toast.error(`${item.name} กรอกได้ไม่เกิน ${item.maxScore} คะแนน`);
				return false;
			}
		}
		const nextValues = { ...scoreValues };
		for (const mutation of mutations) {
			const key = cellKey(mutation.studentId, mutation.itemId);
			nextValues[key] = mutation.value;
			scoreQueue.enqueue(
				mutation.value === null
					? {
							operation: 'clear',
							scoreItemId: mutation.itemId,
							studentAcademicYearId: mutation.studentId,
							rowVersion: scoreVersions[key] ?? null
						}
					: {
							operation: 'set',
							scoreItemId: mutation.itemId,
							studentAcademicYearId: mutation.studentId,
							value: mutation.value,
							rowVersion: scoreVersions[key] ?? null
						}
			);
		}
		scoreValues = nextValues;
		return true;
	}

	function changeEvaluation(criterionId: string, studentId: string, value: number | null): void {
		if (!evaluationWorkspace?.canManage) return;
		if (value !== null && ![0, 1, 2, 3].includes(value)) return;
		const key = cellKey(studentId, criterionId);
		evaluationValues = { ...evaluationValues, [key]: value };
		evaluationQueue.enqueue({
			subjectTermCriterionId: criterionId,
			studentAcademicYearId: studentId,
			qualityLevel: value,
			rowVersion: evaluationVersions[key] ?? null
		});
	}

	function openItemDialog(item: GradebookScoreItem | null): void {
		editingItem = item;
		itemDialogRevision += 1;
		itemDialogOpen = true;
	}

	async function saveItem(input: GradebookItemInput): Promise<void> {
		const context = contextValue();
		if (!context || !scoreWorkspace) return;
		if (!(await flushPendingWork())) return;
		itemBusy = true;
		try {
			const saved = editingItem
				? await updateGradebookItem(selectedGroupId, activePhase, editingItem.id, context, input)
				: await createGradebookItem(selectedGroupId, activePhase, context, input);
			scoreWorkspace = {
				...scoreWorkspace,
				items: editingItem
					? scoreWorkspace.items.map((item) => (item.id === saved.id ? saved : item))
					: [...scoreWorkspace.items, saved],
				confirmationIsCurrent: false
			};
			itemDialogOpen = false;
			selectedItemIds = selectNewGradebookItem(selectedItemIds, saved.id);
			await loadSelectedWorkspace();
			toast.success(editingItem ? 'บันทึกรายการคะแนนแล้ว' : 'เพิ่มรายการคะแนนแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกรายการคะแนนไม่สำเร็จ');
		} finally {
			itemBusy = false;
		}
	}

	async function removeItem(item: GradebookScoreItem): Promise<void> {
		const context = contextValue();
		if (!context || !scoreWorkspace) return;
		if (!(await flushPendingWork())) return;
		itemBusy = true;
		try {
			const outcome = await removeGradebookItem(selectedGroupId, activePhase, item.id, context, {
				rowVersion: item.rowVersion
			});
			scoreWorkspace = {
				...scoreWorkspace,
				items:
					outcome.disposition === 'deleted'
						? scoreWorkspace.items.filter((candidate) => candidate.id !== item.id)
						: scoreWorkspace.items.map((candidate) =>
								candidate.id === item.id
									? { ...candidate, lifecycle: 'cancelled', rowVersion: outcome.rowVersion }
									: candidate
							),
				confirmationIsCurrent: false
			};
			selectedItemIds = selectedItemIds.filter((id) => id !== item.id);
			itemDialogOpen = false;
			await loadSelectedWorkspace();
			toast.success(
				outcome.disposition === 'deleted'
					? 'ลบรายการคะแนนแล้ว'
					: 'ยกเลิกรายการและเก็บคะแนนเดิมไว้แล้ว'
			);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'นำรายการคะแนนออกไม่สำเร็จ');
		} finally {
			itemBusy = false;
		}
	}

	async function loadEvaluationConfiguration(): Promise<void> {
		const context = contextValue();
		if (!context || activeTab === 'scores' || !selectedSubjectId) return;
		const domain = activeTab;
		const subjectId = selectedSubjectId;
		const { revision, signal } = criteriaRequest.begin();
		criteriaBusy = true;
		try {
			const result = await getLearnerEvaluationConfiguration(subjectId, domain, context, {
				signal
			});
			if (criteriaRequest.isCurrent(revision)) evaluationConfiguration = result;
		} catch (error) {
			if (!isAbortError(error)) {
				toast.error(error instanceof Error ? error.message : 'โหลดหัวข้อประเมินไม่สำเร็จ');
			}
		} finally {
			if (criteriaRequest.isCurrent(revision)) criteriaBusy = false;
		}
	}

	function openCriteriaDialog(): void {
		evaluationConfiguration = null;
		criteriaDialogOpen = true;
		void loadEvaluationConfiguration();
	}

	async function createCriterion(name: string): Promise<void> {
		const context = contextValue();
		if (!context || activeTab === 'scores' || !evaluationConfiguration) return;
		if (!(await flushPendingWork())) return;
		criteriaBusy = true;
		try {
			await createSubjectEvaluationCriterion(selectedSubjectId, activeTab, context, {
				name,
				active: true,
				displayOrder:
					Math.max(0, ...evaluationConfiguration.criteria.map((row) => row.displayOrder)) + 1,
				rowVersion: null
			});
			await refreshCriteriaAndWorkspace();
			toast.success('เพิ่มหัวข้อประเมินแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'เพิ่มหัวข้อประเมินไม่สำเร็จ');
		} finally {
			criteriaBusy = false;
		}
	}

	async function updateCriterion(
		criterion: LearnerEvaluationCriterion,
		name: string
	): Promise<void> {
		const context = contextValue();
		if (!context || activeTab === 'scores') return;
		if (!(await flushPendingWork())) return;
		criteriaBusy = true;
		try {
			await updateSubjectEvaluationCriterion(selectedSubjectId, activeTab, criterion.id, context, {
				name,
				active: criterion.lifecycle === 'active',
				displayOrder: criterion.displayOrder,
				rowVersion: criterion.rowVersion
			});
			await refreshCriteriaAndWorkspace();
			toast.success('แก้หัวข้อประเมินแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'แก้หัวข้อประเมินไม่สำเร็จ');
		} finally {
			criteriaBusy = false;
		}
	}

	async function removeCriterion(criterion: LearnerEvaluationCriterion): Promise<void> {
		const context = contextValue();
		if (!context || activeTab === 'scores') return;
		if (!(await flushPendingWork())) return;
		criteriaBusy = true;
		try {
			await removeSubjectEvaluationCriterion(selectedSubjectId, activeTab, criterion.id, context, {
				rowVersion: criterion.rowVersion
			});
			await refreshCriteriaAndWorkspace();
			toast.success('นำหัวข้อประเมินออกแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'นำหัวข้อประเมินออกไม่สำเร็จ');
		} finally {
			criteriaBusy = false;
		}
	}

	async function refreshCriteriaAndWorkspace(): Promise<void> {
		await loadEvaluationConfiguration();
		await loadSelectedWorkspace();
	}

	async function toggleGradebookControl(control: GradebookControl): Promise<void> {
		const context = contextValue();
		if (!context) return;
		if (!(await flushPendingWork())) return;
		controlBusyKey = `gradebook:${control.id}`;
		try {
			const saved = await updateGradebookControl(control.id, context, {
				scoreEntryEnabled: !control.scoreEntryEnabled,
				rowVersion: control.rowVersion
			});
			gradebookControls = gradebookControls.map((row) => (row.id === saved.id ? saved : row));
			if (activeTab === 'scores' && saved.phaseCode === activePhase) {
				await loadSelectedWorkspace();
			}
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกช่วงกรอกคะแนนไม่สำเร็จ');
		} finally {
			controlBusyKey = '';
		}
	}

	async function toggleEvaluationControl(control: LearnerEvaluationControl): Promise<void> {
		const context = contextValue();
		if (!context) return;
		if (!(await flushPendingWork())) return;
		controlBusyKey = `evaluation:${control.id}`;
		try {
			const saved = await updateLearnerEvaluationControl(control.domain, context, {
				entryEnabled: !control.entryEnabled,
				rowVersion: control.rowVersion
			});
			evaluationControls = evaluationControls.map((row) => (row.id === saved.id ? saved : row));
			if (activeTab === saved.domain) await loadSelectedWorkspace();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกช่วงกรอกผลประเมินไม่สำเร็จ');
		} finally {
			controlBusyKey = '';
		}
	}

	async function confirmCurrentWorkspace(): Promise<void> {
		const context = contextValue();
		if (!context || !selectedGroupId) return;
		if (!(await flushPendingWork())) return;
		confirming = true;
		try {
			if (activeTab === 'scores' && scoreWorkspace) {
				const confirmation = await confirmGradebookPhase(selectedGroupId, activePhase, context, {
					sourceChecksum: scoreWorkspace.sourceChecksum,
					rosterChecksum: scoreWorkspace.rosterChecksum,
					rowVersion: scoreWorkspace.confirmation?.rowVersion ?? null
				});
				scoreWorkspace = {
					...scoreWorkspace,
					confirmation,
					confirmationIsCurrent: true
				};
				toast.success('ยืนยันคะแนนช่วงนี้แล้ว');
			} else if (activeTab !== 'scores' && evaluationWorkspace) {
				const result = await confirmLearnerEvaluationGroup(selectedGroupId, activeTab, context, {
					sourceChecksum: evaluationWorkspace.sourceChecksum,
					rosterChecksum: evaluationWorkspace.rosterChecksum,
					rowVersion: evaluationWorkspace.confirmation?.rowVersion ?? null
				});
				if (result.confirmation) {
					evaluationWorkspace = {
						...evaluationWorkspace,
						confirmation: result.confirmation,
						confirmationIsCurrent: true
					};
					toast.success('ยืนยันผลประเมินกลุ่มนี้แล้ว');
				} else {
					toast.warning(`ยังมีผลประเมินว่าง ${result.missing.length} ช่อง`);
				}
			}
			confirmationDialogOpen = false;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ยืนยันข้อมูลไม่สำเร็จ');
		} finally {
			confirming = false;
		}
	}

	function changeMobileValue(value: string): void {
		if (!mobileCell) return;
		if (mobileCell.mode === 'score') {
			applyScoreMutations([
				{
					itemId: mobileCell.itemId,
					studentId: mobileCell.studentId,
					value: value.trim() || null
				}
			]);
			return;
		}
		changeEvaluation(
			mobileCell.criterionId,
			mobileCell.studentId,
			value === 'blank' ? null : Number(value)
		);
	}

	async function closeMobileEditor(): Promise<void> {
		if (await flushPendingWork()) mobileCell = null;
	}

	function errorToast(message: string): void {
		toast.error(message);
	}

	onMount(() => {
		let loadedContextKey = '';
		const unsubscribeScore = scoreQueue.subscribe((snapshot) => (scoreSaveStatus = snapshot));
		const unsubscribeEvaluation = evaluationQueue.subscribe(
			(snapshot) => (evaluationSaveStatus = snapshot)
		);
		const unregisterDirty = registerAcademicContextDirtySource('academic-gradebook-entry', () =>
			[scoreQueue.status(), evaluationQueue.status()].some((snapshot) => snapshot.state !== 'saved')
		);
		const unsubscribeContext = academicContext.subscribe((state) => {
			const yearId = state.selected.academicYearId;
			const termId = state.selected.academicTermId;
			const contextKey = yearId && termId ? `${yearId}:${termId}` : '';
			if (yearId && termId && contextKey !== loadedContextKey) {
				loadedContextKey = contextKey;
				scoreQueue.discard();
				evaluationQueue.discard();
				workspaceRequest.abort();
				criteriaRequest.abort();
				scoreWorkspace = null;
				evaluationWorkspace = null;
				evaluationConfiguration = null;
				gradebookControls = [];
				evaluationControls = [];
				const context = { academicYearId: yearId, academicTermId: termId };
				void loadManagerControls(context);
				void loadSubjects(context);
			} else if (!contextKey) {
				loadedContextKey = '';
				subjectsRequest.abort();
				workspaceRequest.abort();
				criteriaRequest.abort();
				scoreSubjects = [];
				evaluationSubjects = [];
				scoreWorkspace = null;
				evaluationWorkspace = null;
				loading = false;
			}
		});

		return () => {
			subjectsRequest.abort();
			workspaceRequest.abort();
			criteriaRequest.abort();
			unsubscribeScore();
			unsubscribeEvaluation();
			unsubscribeContext();
			unregisterDirty();
		};
	});
</script>

<PageShell
	title="กรอกคะแนนและประเมินผู้เรียน"
	description="สมุดบันทึกของครูสำหรับคะแนนรายวิชา คุณลักษณะ และการอ่าน คิดวิเคราะห์ และเขียน"
>
	{#if !canReadScores && !canReadEvaluations}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูสมุดคะแนนหรือผลประเมิน"
			description="ติดต่อผู้ดูแลเพื่อขอสิทธิ์ตามรายวิชาที่รับผิดชอบหรือขอบเขตงานวิชาการ"
		/>
	{:else if !academicYearId || !academicTermId}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาและภาคเรียนก่อน"
			description="ใช้ตัวเลือกบนแถบด้านบนเพื่อเปิดสมุดบันทึกของภาคเรียนที่ต้องการ"
		/>
	{:else if loading}
		<div class="space-y-4">
			<PageSkeleton variant="form" rows={2} />
			<PageSkeleton variant="table" rows={8} columns={6} />
		</div>
	{:else if errorMessage && scoreSubjects.length === 0 && evaluationSubjects.length === 0}
		<PageState
			variant="error"
			title="โหลดสมุดบันทึกไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => {
				const context = contextValue();
				if (context) void loadSubjects(context);
			}}
		/>
	{:else}
		<div class="space-y-4">
			<div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_auto]">
				<GradebookWorkspaceHeader
					subjects={activeSubjects}
					{selectedSubjectId}
					{selectedGroupId}
					disabled={activeSaveStatus.state === 'saving'}
					onselect={(subjectId, groupId) => void changeSelection(subjectId, groupId)}
				/>
				<Card.Root class="gap-0 py-0 xl:w-72">
					<Card.Content class="flex h-full items-center gap-3 px-4 py-3">
						<div
							class={[
								'flex size-9 shrink-0 items-center justify-center rounded-lg',
								activeSaveStatus.state === 'failed'
									? 'bg-destructive/10 text-destructive'
									: activeSaveStatus.state === 'unsaved'
										? 'bg-amber-100 text-amber-700'
										: 'bg-emerald-100 text-emerald-700'
							]}
						>
							{#if activeSaveStatus.state === 'saving'}
								<Loader2 class="size-4 animate-spin" />
							{:else if activeSaveStatus.state === 'failed'}
								<AlertCircle class="size-4" />
							{:else if activeSaveStatus.state === 'unsaved'}
								<Cloud class="size-4" />
							{:else}
								<Check class="size-4" />
							{/if}
						</div>
						<div class="min-w-0 flex-1">
							<p class="text-sm font-medium">
								{activeSaveStatus.state === 'saving'
									? 'กำลังบันทึก'
									: activeSaveStatus.state === 'failed'
										? 'บันทึกไม่สำเร็จ'
										: activeSaveStatus.state === 'unsaved'
											? 'มีข้อมูลรอบันทึก'
											: 'บันทึกแล้ว'}
							</p>
							<p class="truncate text-xs text-muted-foreground">
								{activeSaveStatus.pendingCount > 0
									? `${activeSaveStatus.pendingCount} ช่อง`
									: 'บันทึกอัตโนมัติ'}
							</p>
						</div>
						{#if activeSaveStatus.state === 'failed'}
							<Button
								size="icon-sm"
								variant="ghost"
								aria-label="ลองบันทึกอีกครั้ง"
								onclick={() => void retryPendingWork()}
							>
								<RotateCcw class="size-4" />
							</Button>
						{/if}
					</Card.Content>
				</Card.Root>
			</div>

			<Tabs.Root value={activeTab} onValueChange={(value) => void changeTab(value)}>
				<Tabs.List class="grid h-auto w-full grid-cols-1 gap-1 p-1 sm:grid-cols-3">
					<Tabs.Trigger value="scores" disabled={!canReadScores} class="min-h-10">
						<ClipboardCheck class="size-4" /> คะแนนรายวิชา
					</Tabs.Trigger>
					<Tabs.Trigger
						value="desirable_characteristic"
						disabled={!canReadEvaluations}
						class="min-h-10"
					>
						<ShieldCheck class="size-4" /> คุณลักษณะอันพึงประสงค์
					</Tabs.Trigger>
					<Tabs.Trigger
						value="reading_thinking_writing"
						disabled={!canReadEvaluations}
						class="min-h-10"
					>
						<BookOpenCheck class="size-4" /> การอ่าน คิดวิเคราะห์ และเขียน
					</Tabs.Trigger>
				</Tabs.List>
			</Tabs.Root>

			{#if canManageGradebookSchool || canManageEvaluationSchool}
				<GradebookEntryControls
					{gradebookControls}
					{evaluationControls}
					canManageGradebook={canManageGradebookSchool}
					canManageEvaluation={canManageEvaluationSchool}
					busyKey={controlBusyKey}
					ontoggleGradebook={(control) => void toggleGradebookControl(control)}
					ontoggleEvaluation={(control) => void toggleEvaluationControl(control)}
				/>
			{/if}

			{#if activeSaveStatus.state === 'failed'}
				<div
					class="flex flex-col gap-3 rounded-xl border border-destructive/30 bg-destructive/5 p-4 sm:flex-row sm:items-center"
				>
					<div class="min-w-0 flex-1">
						<p class="font-medium text-destructive">ข้อมูลยังอยู่ในหน้านี้ แต่ยังบันทึกไม่สำเร็จ</p>
						<p class="text-sm text-muted-foreground">
							ลองบันทึกอีกครั้ง หรือละทิ้งข้อมูลที่ค้างแล้วโหลดค่าจากระบบใหม่
						</p>
					</div>
					<div class="flex gap-2">
						<Button variant="outline" onclick={discardPendingWork}
							><Trash2 class="size-4" /> ละทิ้ง</Button
						>
						<Button onclick={() => void retryPendingWork()}
							><RotateCcw class="size-4" /> ลองใหม่</Button
						>
					</div>
				</div>
			{/if}

			{#if activeTab === 'scores'}
				<nav
					class="grid grid-cols-2 gap-2 rounded-xl border bg-card p-2 sm:grid-cols-4"
					aria-label="ช่วงคะแนน"
				>
					{#each phaseCodes as phase (phase)}
						<Button
							variant={activePhase === phase ? 'default' : 'ghost'}
							class="justify-between"
							disabled={activeSaveStatus.state === 'saving'}
							onclick={() => void changePhase(phase)}
						>
							{phaseLabels[phase]}
							{#if selectedScoreSubject?.phases.some((row) => row.phaseCode === phase)}
								<span class="text-xs opacity-75"
									>{selectedScoreSubject.phases.find((row) => row.phaseCode === phase)
										?.maxScore}</span
								>
							{/if}
						</Button>
					{/each}
				</nav>
			{/if}

			{#if activeSubjects.length === 0}
				<AcademicPrerequisiteNotice
					prerequisite={{
						key: 'gradebook-subjects',
						status: 'missing',
						title:
							activeTab === 'scores'
								? 'ยังไม่มีรายวิชาสำหรับกรอกคะแนน'
								: 'ยังไม่มีรายวิชาสำหรับประเมินผู้เรียน',
						description: 'ตรวจรายการเปิดสอน กลุ่มเรียน ครู และโครงสร้างคะแนนของภาคเรียนนี้ก่อน',
						actionLabel: 'ไปจัดรายการเปิดสอน',
						href: '/staff/academic/delivery'
					}}
				/>
			{:else if activeTab === 'scores' && !selectedPhaseAvailable}
				<AcademicPrerequisiteNotice
					prerequisite={{
						key: `gradebook-phase-${activePhase}`,
						status: 'missing',
						title: `ยังไม่มีโครงสร้างคะแนน${phaseLabels[activePhase]}`,
						description: 'กำหนดคะแนนเต็มและรูปแบบการประเมินของช่วงนี้ก่อนสร้างรายการคะแนนย่อย',
						actionLabel: 'ไปหน้าโครงสร้างคะแนน',
						href: '/staff/academic/assessments'
					}}
				/>
			{:else if workspaceLoading}
				<PageSkeleton variant="table" rows={10} columns={7} />
			{:else if workspaceError}
				<PageState
					variant="error"
					title="โหลดพื้นที่กรอกข้อมูลไม่สำเร็จ"
					description={workspaceError}
					actionLabel="ลองอีกครั้ง"
					onaction={() => void loadSelectedWorkspace()}
				/>
			{:else if activeTab === 'scores' && scoreWorkspace}
				<ScoreLedger
					workspace={scoreWorkspace}
					values={scoreValues}
					{selectedItemIds}
					canManage={scoreWorkspace.canManage}
					disabled={activeSaveStatus.state === 'failed' || scoreWorkspace.locked}
					onselectionchange={(ids) => (selectedItemIds = ids)}
					onmutations={applyScoreMutations}
					onflush={async () => {
						await scoreQueue.flush();
					}}
					onopenitem={openItemDialog}
					onopenmobile={(position: GradebookCellPosition) =>
						(mobileCell = { mode: 'score', ...position })}
					onconfirm={() => (confirmationDialogOpen = true)}
					onerror={errorToast}
				/>
			{:else if activeTab !== 'scores' && evaluationWorkspace}
				<LearnerEvaluationLedger
					workspace={evaluationWorkspace}
					values={evaluationValues}
					{selectedCriterionIds}
					disabled={activeSaveStatus.state === 'failed' || evaluationWorkspace.locked}
					onselectionchange={(ids) => (selectedCriterionIds = ids)}
					onchange={changeEvaluation}
					onopencriteria={openCriteriaDialog}
					onopenmobile={(criterionId, studentId) =>
						(mobileCell = { mode: 'evaluation', criterionId, studentId })}
					onconfirm={() => (confirmationDialogOpen = true)}
				/>
			{/if}
		</div>
	{/if}
</PageShell>

{#if itemDialogOpen}
	{#key itemDialogRevision}
		<ScoreItemDialog
			open={itemDialogOpen}
			item={editingItem}
			nextDisplayOrder={nextItemDisplayOrder}
			busy={itemBusy}
			onopenchange={(open) => (itemDialogOpen = open)}
			onsave={(input) => void saveItem(input)}
			onremove={(item) => void removeItem(item)}
		/>
	{/key}
{/if}

<SubjectCriteriaDialog
	open={criteriaDialogOpen}
	configuration={evaluationConfiguration}
	busy={criteriaBusy}
	onopenchange={(open) => (criteriaDialogOpen = open)}
	oncreate={(name) => void createCriterion(name)}
	onupdate={(criterion, name) => void updateCriterion(criterion, name)}
	onremove={(criterion) => void removeCriterion(criterion)}
/>

<PhaseConfirmationDialog
	open={confirmationDialogOpen}
	title={activeTab === 'scores'
		? `ยืนยันคะแนน${phaseLabels[activePhase]}`
		: `ยืนยัน${tabLabels[activeTab]}`}
	description={activeTab === 'scores'
		? 'ระบบจะบันทึกภาพรวมคะแนนของห้องนี้เพื่อเตรียมตัดผลการเรียน'
		: 'ระบบจะยืนยันว่าครูหลักตรวจผลประเมินทุกหัวข้อของห้องนี้แล้ว'}
	blankCount={activeTab === 'scores' ? scoreBlankCount : 0}
	busy={confirming}
	onopenchange={(open) => (confirmationDialogOpen = open)}
	onconfirm={() => void confirmCurrentWorkspace()}
/>

{#if mobileContext}
	<GradebookMobileEditor
		open={mobileCell !== null}
		mode={mobileContext.mode}
		studentName={mobileContext.studentName}
		itemName={mobileContext.itemName}
		value={mobileContext.value}
		maxScore={mobileContext.maxScore}
		options={mobileContext.mode === 'evaluation' ? evaluationOptions : []}
		saveStatus={mobileContext.mode === 'score' ? scoreSaveStatus : evaluationSaveStatus}
		onchange={changeMobileValue}
		onrequestclose={() => void closeMobileEditor()}
		onretry={() => void retryPendingWork()}
	/>
{/if}
