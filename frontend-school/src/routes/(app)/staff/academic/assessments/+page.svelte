<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		bulkSaveAssessmentQuickScores,
		getAssessmentSettings,
		listAssessmentPlans,
		updateAssessmentSettings,
		type AssessmentExamMode,
		type AssessmentPlanStatus,
		type AssessmentPlanSummary,
		type AssessmentQuickScoreSaveResult,
		type SaveAssessmentQuickScoreEntryRequest
	} from '$lib/api/academicAssessments';
	import {
		getAcademicStructure,
		listClassrooms,
		type AcademicStructureData,
		type Classroom
	} from '$lib/api/academic';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Switch } from '$lib/components/ui/switch';
	import * as Table from '$lib/components/ui/table';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		AlertTriangle,
		ClipboardList,
		Download,
		FileSpreadsheet,
		Loader2,
		Save
	} from 'lucide-svelte';

	let { data } = $props();

	type StatusFilter = AssessmentPlanStatus | 'all';
	type QuickExamMode = Extract<AssessmentExamMode, 'none' | 'in_timetable' | 'outside_timetable'>;
	type QuickScoreField = 'beforeMidtermScore' | 'midtermScore' | 'afterMidtermScore' | 'finalScore';
	type QuickDurationField = 'midtermExamDurationMinutes' | 'finalExamDurationMinutes';
	type QuickValidationField = QuickScoreField | QuickDurationField;
	type QuickExamModeField = 'midtermExamMode' | 'finalExamMode';
	type QuickScoreColumn = { field: QuickScoreField; heading: string; label: string };
	type QuickScoreDraft = {
		beforeMidtermScore: number | null;
		midtermScore: number | null;
		afterMidtermScore: number | null;
		finalScore: number | null;
		midtermExamMode: QuickExamMode;
		finalExamMode: QuickExamMode;
		midtermExamDurationMinutes: number | null;
		finalExamDurationMinutes: number | null;
		dirty: boolean;
	};
	type QuickScoreValidationIssue = {
		planKey: string;
		field: QuickValidationField;
		message: string;
	};

	let teacherAccessEnabled = $state(true);

	const canReadAssessment = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_ASSESSMENT_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_ASSESSMENT_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_COURSE_PLAN_READ_ALL,
			PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL
		)
	);
	const canReadSchoolAssessment = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_ASSESSMENT_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_COURSE_PLAN_READ_ALL,
			PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL
		)
	);
	const canManageSchoolAssessment = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL
		)
	);
	const canManageAssessment = $derived(
		canManageSchoolAssessment ||
			(teacherAccessEnabled && $can.has(PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED))
	);

	const examModeOptions: { value: AssessmentExamMode; label: string }[] = [
		{ value: 'none', label: 'ไม่ใช่การสอบ' },
		{ value: 'in_timetable', label: 'สอบในตาราง' },
		{ value: 'outside_timetable', label: 'สอบนอกตาราง' },
		{ value: 'practical', label: 'ปฏิบัติ/ชิ้นงาน' }
	];

	const statusOptions: { value: StatusFilter; label: string }[] = [
		{ value: 'all', label: 'ทุกสถานะ' },
		{ value: 'not_configured', label: 'ยังไม่ตั้งค่า' },
		{ value: 'draft', label: 'ร่าง' },
		{ value: 'saved', label: 'บันทึกแล้ว' },
		{ value: 'submitted', label: 'ส่งแล้ว' },
		{ value: 'locked', label: 'ล็อกแล้ว' }
	];
	const quickExamModeOptions: { value: QuickExamMode; label: string }[] = [
		{ value: 'none', label: 'ไม่มีสอบ' },
		{ value: 'in_timetable', label: 'ในตาราง' },
		{ value: 'outside_timetable', label: 'นอกตาราง' }
	];
	const quickScoreColumns: QuickScoreColumn[] = [
		{ field: 'beforeMidtermScore', heading: 'ก่อน', label: 'ก่อนกลางภาค' },
		{ field: 'midtermScore', heading: 'กลาง', label: 'กลางภาค' },
		{ field: 'afterMidtermScore', heading: 'หลัง', label: 'หลังกลางภาค' },
		{ field: 'finalScore', heading: 'ปลาย', label: 'ปลายภาค' }
	];
	let loading = $state(true);
	let loadingPlans = $state(false);
	let settingsLoading = $state(false);
	let settingsSaving = $state(false);
	let exporting = $state(false);
	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let classrooms = $state<Classroom[]>([]);
	let plans = $state<AssessmentPlanSummary[]>([]);
	let selectedYearId = $state('');
	let selectedSemesterId = $state('');
	let selectedClassroomId = $state('');
	let selectedStatus = $state<StatusFilter>('all');
	let savingAllQuickScores = $state(false);
	let quickScoreDrafts = $state<Record<string, QuickScoreDraft>>({});

	const filteredSemesters = $derived(
		structure.semesters.filter((semester) => semester.academic_year_id === selectedYearId)
	);
	const teacherAccessBlocked = $derived(!canReadSchoolAssessment && !teacherAccessEnabled);
	const hasDirtyQuickScoreDrafts = $derived(
		Object.values(quickScoreDrafts).some((draft) => draft.dirty)
	);

	const summary = $derived({
		total: plans.length,
		draft: plans.filter((plan) => plan.status === 'draft' || plan.status === 'not_configured')
			.length,
		saved: plans.filter((plan) => plan.status === 'saved').length,
		submitted: plans.filter((plan) => plan.status === 'submitted').length,
		locked: plans.filter((plan) => plan.status === 'locked').length,
		outside: plans.reduce((total, plan) => total + plan.outsideTimetableCount, 0),
		unallocated: plans.filter((plan) => plan.hasUnallocatedCategories).length
	});

	function statusLabel(status: AssessmentPlanStatus) {
		return statusOptions.find((option) => option.value === status)?.label ?? status;
	}

	function statusBadgeVariant(status: AssessmentPlanStatus) {
		if (status === 'saved') return 'default';
		if (status === 'submitted') return 'default';
		if (status === 'locked') return 'secondary';
		if (status === 'not_configured') return 'outline';
		return 'secondary';
	}

	function examModeLabel(mode: AssessmentExamMode | string) {
		return examModeOptions.find((option) => option.value === mode)?.label ?? mode;
	}

	function courseTitle(plan: AssessmentPlanSummary) {
		const subject = [plan.subjectCode, plan.subjectNameTh || plan.subjectNameEn]
			.filter(Boolean)
			.join(' ');
		return subject || 'รายวิชา';
	}

	function compareNullableNumber(left?: number | null, right?: number | null) {
		const leftValue = left ?? Number.MAX_SAFE_INTEGER;
		const rightValue = right ?? Number.MAX_SAFE_INTEGER;
		return leftValue - rightValue;
	}

	function compareExportText(left?: string | null, right?: string | null) {
		return (left ?? '').localeCompare(right ?? '', 'th', {
			numeric: true,
			sensitivity: 'base'
		});
	}

	function gradeLevelLabel(plan: AssessmentPlanSummary) {
		const prefixBySort: Record<number, string> = {
			1: 'อ.',
			2: 'ป.',
			3: 'ม.'
		};
		const prefix = prefixBySort[plan.gradeLevelSort] ?? '';
		return prefix ? `${prefix}${plan.gradeYear}` : '';
	}

	function sortedAssessmentExportPlans(sourcePlans: AssessmentPlanSummary[]) {
		return [...sourcePlans].sort((left, right) => {
			return (
				compareNullableNumber(left.subjectGroupDisplayOrder, right.subjectGroupDisplayOrder) ||
				compareExportText(left.subjectGroupName, right.subjectGroupName) ||
				compareNullableNumber(left.gradeLevelSort, right.gradeLevelSort) ||
				compareNullableNumber(left.gradeYear, right.gradeYear) ||
				compareExportText(left.classroomRoomNumber, right.classroomRoomNumber) ||
				compareExportText(left.subjectCode, right.subjectCode) ||
				compareExportText(courseTitle(left), courseTitle(right))
			);
		});
	}

	function assessmentPlanKey(plan: AssessmentPlanSummary) {
		return `${plan.academicSemesterId}-${plan.subjectId}`;
	}

	function classroomSummary(plan: AssessmentPlanSummary) {
		if (!plan.classroomName) return '-';
		if (plan.classroomCount <= 1) return plan.classroomName;
		return `${plan.classroomName} (${plan.classroomCount} ห้อง)`;
	}

	function quickExamMode(value?: string | null): QuickExamMode {
		if (value === 'none' || value === 'outside_timetable') return value;
		return 'in_timetable';
	}

	function canEditExamDuration(mode: AssessmentExamMode | string) {
		return mode === 'in_timetable';
	}

	function canEditAssessmentPlan(plan: AssessmentPlanSummary) {
		return canManageAssessment && plan.canManage && plan.status !== 'locked';
	}

	function planMatchesStatusFilter(plan: AssessmentPlanSummary) {
		return selectedStatus === 'all' || plan.status === selectedStatus;
	}

	function quickScoreDraftValue(plan: AssessmentPlanSummary, value: number) {
		if (plan.status === 'not_configured' && value === 0) return null;
		return value;
	}

	function quickScoreDraftFromPlan(plan: AssessmentPlanSummary): QuickScoreDraft {
		return {
			beforeMidtermScore: quickScoreDraftValue(plan, plan.beforeMidtermScore),
			midtermScore: quickScoreDraftValue(plan, plan.midtermScore),
			afterMidtermScore: quickScoreDraftValue(plan, plan.afterMidtermScore),
			finalScore: quickScoreDraftValue(plan, plan.finalScore),
			midtermExamMode: quickExamMode(plan.midtermExamMode),
			finalExamMode: quickExamMode(plan.finalExamMode),
			midtermExamDurationMinutes: plan.midtermExamDurationMinutes ?? null,
			finalExamDurationMinutes: plan.finalExamDurationMinutes ?? null,
			dirty: false
		};
	}

	function syncQuickScoreDrafts(nextPlans: AssessmentPlanSummary[]) {
		const nextDrafts: Record<string, QuickScoreDraft> = {};
		for (const plan of nextPlans) {
			const key = assessmentPlanKey(plan);
			const existing = quickScoreDrafts[key];
			nextDrafts[key] = existing && existing.dirty ? existing : quickScoreDraftFromPlan(plan);
		}
		quickScoreDrafts = nextDrafts;
	}

	function quickDraftForPlan(plan: AssessmentPlanSummary) {
		const key = assessmentPlanKey(plan);
		quickScoreDrafts[key] ??= quickScoreDraftFromPlan(plan);
		return quickScoreDrafts[key];
	}

	function parseQuickNumber(value: string) {
		if (value.trim() === '') return null;
		const parsed = Number.parseFloat(value);
		return Number.isFinite(parsed) ? parsed : null;
	}

	function setQuickScoreValue(plan: AssessmentPlanSummary, field: QuickScoreField, value: string) {
		const draft = quickDraftForPlan(plan);
		draft[field] = parseQuickNumber(value);
		draft.dirty = true;
	}

	function setQuickDurationValue(
		plan: AssessmentPlanSummary,
		field: QuickDurationField,
		value: string
	) {
		const draft = quickDraftForPlan(plan);
		const duration = parseQuickNumber(value);
		draft[field] = duration == null ? null : Math.max(1, Math.trunc(duration));
		draft.dirty = true;
	}

	function setQuickExamModeValue(
		plan: AssessmentPlanSummary,
		field: QuickExamModeField,
		value: string
	) {
		const draft = quickDraftForPlan(plan);
		draft[field] = quickExamMode(value);
		if (field === 'midtermExamMode' && !canEditExamDuration(draft.midtermExamMode)) {
			draft.midtermExamDurationMinutes = null;
		}
		if (field === 'finalExamMode' && !canEditExamDuration(draft.finalExamMode)) {
			draft.finalExamDurationMinutes = null;
		}
		draft.dirty = true;
	}

	function handleQuickScoreEnter(event: KeyboardEvent) {
		if (event.key !== 'Enter') return;
		const currentInput = event.currentTarget;
		if (!(currentInput instanceof HTMLInputElement)) return;
		event.preventDefault();

		const inputScope = currentInput.closest('table') ?? document;
		const scoreInputs = Array.from(
			inputScope.querySelectorAll<HTMLInputElement>(
				'input[data-assessment-quick-score-input]:not(:disabled)'
			)
		);
		const currentIndex = scoreInputs.indexOf(currentInput);
		const nextInput = scoreInputs[currentIndex + 1];
		nextInput?.focus();
		nextInput?.select();
	}

	function firstQuickScoreValidationIssue(
		targetPlans: AssessmentPlanSummary[]
	): QuickScoreValidationIssue | null {
		for (const plan of targetPlans) {
			const planKey = assessmentPlanKey(plan);
			const draft = quickScoreDrafts[planKey];
			if (!draft) continue;

			for (const column of quickScoreColumns) {
				if (draft[column.field] == null) {
					return {
						planKey,
						field: column.field,
						message: `กรุณากรอกคะแนน${column.label}: ${courseTitle(plan)}`
					};
				}
			}

			if (canEditExamDuration(draft.midtermExamMode) && draft.midtermExamDurationMinutes == null) {
				return {
					planKey,
					field: 'midtermExamDurationMinutes',
					message: `กรุณากรอกระยะเวลากลางภาค: ${courseTitle(plan)}`
				};
			}

			if (canEditExamDuration(draft.finalExamMode) && draft.finalExamDurationMinutes == null) {
				return {
					planKey,
					field: 'finalExamDurationMinutes',
					message: `กรุณากรอกระยะเวลาปลายภาค: ${courseTitle(plan)}`
				};
			}
		}

		return null;
	}

	function focusQuickScoreValidationIssue(issue: QuickScoreValidationIssue) {
		const input = Array.from(
			document.querySelectorAll<HTMLInputElement>(
				'input[data-assessment-plan-key][data-assessment-field]'
			)
		).find(
			(candidate) =>
				candidate.dataset.assessmentPlanKey === issue.planKey &&
				candidate.dataset.assessmentField === issue.field
		);
		input?.scrollIntoView({ block: 'nearest', inline: 'center' });
		input?.focus();
		input?.select();
	}

	function showQuickScoreValidationIssue(issue: QuickScoreValidationIssue) {
		toast.error(issue.message);
		focusQuickScoreValidationIssue(issue);
	}

	function quickScoreValue(value: number | null) {
		return Number(value ?? 0);
	}

	function scoreToSaveValue(value: number | null) {
		if (value == null) {
			throw new Error('คะแนนยังไม่ครบ');
		}
		return Number(value);
	}

	function quickDurationValue(value: number | null) {
		if (value == null) return null;
		const duration = Math.trunc(Number(value));
		return duration > 0 ? duration : null;
	}

	function quickScoreTotal(draft: QuickScoreDraft) {
		return (
			quickScoreValue(draft.beforeMidtermScore) +
			quickScoreValue(draft.midtermScore) +
			quickScoreValue(draft.afterMidtermScore) +
			quickScoreValue(draft.finalScore)
		);
	}

	function buildQuickScoreEntry(
		plan: AssessmentPlanSummary,
		draft: QuickScoreDraft
	): SaveAssessmentQuickScoreEntryRequest {
		return {
			classroomCourseId: plan.classroomCourseId,
			beforeMidtermScore: scoreToSaveValue(draft.beforeMidtermScore),
			midtermScore: scoreToSaveValue(draft.midtermScore),
			afterMidtermScore: scoreToSaveValue(draft.afterMidtermScore),
			finalScore: scoreToSaveValue(draft.finalScore),
			midtermExamMode: draft.midtermExamMode,
			midtermExamDurationMinutes: canEditExamDuration(draft.midtermExamMode)
				? quickDurationValue(draft.midtermExamDurationMinutes)
				: null,
			finalExamMode: draft.finalExamMode,
			finalExamDurationMinutes: canEditExamDuration(draft.finalExamMode)
				? quickDurationValue(draft.finalExamDurationMinutes)
				: null
		};
	}

	function patchQuickScoreSaveResults(results: AssessmentQuickScoreSaveResult[]) {
		const resultsByCourseId = new Map(results.map((result) => [result.classroomCourseId, result]));
		const nextDrafts = { ...quickScoreDrafts };
		const patchedPlans = plans.map((plan) => {
			const result = resultsByCourseId.get(plan.classroomCourseId);
			if (!result) return plan;
			const key = assessmentPlanKey(plan);
			const draft = nextDrafts[key];
			if (draft) {
				draft.beforeMidtermScore = result.beforeMidtermScore;
				draft.midtermScore = result.midtermScore;
				draft.afterMidtermScore = result.afterMidtermScore;
				draft.finalScore = result.finalScore;
				draft.midtermExamMode = quickExamMode(result.midtermExamMode);
				draft.finalExamMode = quickExamMode(result.finalExamMode);
				draft.midtermExamDurationMinutes = result.midtermExamDurationMinutes ?? null;
				draft.finalExamDurationMinutes = result.finalExamDurationMinutes ?? null;
				draft.dirty = false;
			}
			return {
				...plan,
				status: result.status,
				categoryCount: result.categoryCount,
				itemCount: result.itemCount,
				totalScore: result.totalScore,
				beforeMidtermScore: result.beforeMidtermScore,
				midtermScore: result.midtermScore,
				afterMidtermScore: result.afterMidtermScore,
				finalScore: result.finalScore,
				outsideTimetableCount: result.outsideTimetableCount,
				inTimetableCount: result.inTimetableCount,
				midtermExamMode: result.midtermExamMode,
				finalExamMode: result.finalExamMode,
				midtermExamDurationMinutes: result.midtermExamDurationMinutes ?? null,
				finalExamDurationMinutes: result.finalExamDurationMinutes ?? null,
				hasUnallocatedCategories: result.hasUnallocatedCategories
			};
		});
		plans = patchedPlans.filter(planMatchesStatusFilter);
		quickScoreDrafts = nextDrafts;
	}

	async function initData() {
		if (!canReadAssessment) {
			loading = false;
			return;
		}
		loading = true;
		try {
			await loadAssessmentSettings();
			if (teacherAccessBlocked) {
				plans = [];
				return;
			}
			const structureResponse = await getAcademicStructure();
			structure = structureResponse.data;
			const activeYear = structure.years.find((year) => year.is_active) ?? structure.years[0];
			selectedYearId = activeYear?.id ?? '';
			const firstSemester =
				structure.semesters.find(
					(semester) => semester.academic_year_id === selectedYearId && semester.is_active
				) ?? structure.semesters.find((semester) => semester.academic_year_id === selectedYearId);
			selectedSemesterId = firstSemester?.id ?? '';
			await loadClassrooms();
			await loadPlans();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	}

	async function loadAssessmentSettings() {
		settingsLoading = true;
		try {
			const response = await getAssessmentSettings();
			teacherAccessEnabled = response.data.teacherAccessEnabled;
		} finally {
			settingsLoading = false;
		}
	}

	async function loadClassrooms() {
		if (!selectedYearId) {
			classrooms = [];
			return;
		}
		const response = await listClassrooms({ year_id: selectedYearId });
		classrooms = response.data ?? [];
	}

	async function loadPlans() {
		if (!canReadAssessment || teacherAccessBlocked) {
			plans = [];
			return;
		}
		loadingPlans = true;
		try {
			const response = await listAssessmentPlans({
				academicSemesterId: selectedSemesterId || undefined,
				classroomId: selectedClassroomId || undefined,
				status: selectedStatus === 'all' ? undefined : selectedStatus
			});
			plans = response.data;
			syncQuickScoreDrafts(response.data);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถโหลดโครงสร้างคะแนนได้');
		} finally {
			loadingPlans = false;
		}
	}

	async function onYearChange(yearId: string) {
		selectedYearId = yearId;
		selectedClassroomId = '';
		const firstSemester = structure.semesters.find(
			(semester) => semester.academic_year_id === yearId
		);
		selectedSemesterId = firstSemester?.id ?? '';
		await loadClassrooms();
		await loadPlans();
	}

	async function toggleTeacherAccess(enabled: boolean) {
		if (!canManageSchoolAssessment || settingsSaving) return;
		settingsSaving = true;
		try {
			const response = await updateAssessmentSettings({ teacherAccessEnabled: enabled });
			teacherAccessEnabled = response.data.teacherAccessEnabled;
			toast.success(teacherAccessEnabled ? 'เปิดให้ครูกรอกแล้ว' : 'ปิดการกรอกของครูแล้ว');
			await loadPlans();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถเปลี่ยนสถานะได้');
		} finally {
			settingsSaving = false;
		}
	}

	async function saveAllQuickScoreRows() {
		if (!canManageAssessment || savingAllQuickScores) return;
		const dirtyPlans = plans
			.filter(canEditAssessmentPlan)
			.filter((plan) => quickScoreDrafts[assessmentPlanKey(plan)]?.dirty);
		if (dirtyPlans.length === 0) return;
		const validationIssue = firstQuickScoreValidationIssue(dirtyPlans);
		if (validationIssue) {
			showQuickScoreValidationIssue(validationIssue);
			return;
		}
		savingAllQuickScores = true;
		try {
			const response = await bulkSaveAssessmentQuickScores({
				plans: dirtyPlans.map((plan) => buildQuickScoreEntry(plan, quickDraftForPlan(plan)))
			});
			patchQuickScoreSaveResults(response.data.plans);
			toast.success('บันทึกการเปลี่ยนแปลงแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถบันทึกการเปลี่ยนแปลงได้');
		} finally {
			savingAllQuickScores = false;
		}
	}

	async function exportAssessmentReport(kind: 'overview' | 'exam') {
		if (plans.length === 0) {
			toast.error('ไม่มีข้อมูลสำหรับดาวน์โหลด');
			return;
		}
		exporting = true;
		try {
			const XLSX = await import('xlsx');
			const rows = sortedAssessmentExportPlans(plans)
				.filter(
					(plan) =>
						kind === 'overview' || plan.inTimetableCount > 0 || plan.outsideTimetableCount > 0
				)
				.map((plan) => ({
					กลุ่มสาระ: plan.subjectGroupName ?? '',
					ระดับชั้น: gradeLevelLabel(plan),
					ห้องเรียนที่เปิด: plan.classroomName ?? '',
					จำนวนห้อง: plan.classroomCount,
					รหัสวิชา: plan.subjectCode ?? '',
					รายวิชา: plan.subjectNameTh ?? plan.subjectNameEn ?? '',
					ครูผู้สอน: plan.instructorName ?? '',
					สถานะ: statusLabel(plan.status),
					คะแนนรวม: plan.totalScore,
					ก่อนกลางภาค: plan.beforeMidtermScore,
					กลางภาค: plan.midtermScore,
					หลังกลางภาค: plan.afterMidtermScore,
					ปลายภาค: plan.finalScore,
					จำนวนหมวด: plan.categoryCount,
					จำนวนคะแนนย่อย: plan.itemCount,
					สอบในตาราง: plan.inTimetableCount,
					สอบนอกตาราง: plan.outsideTimetableCount,
					รูปแบบกลางภาค: examModeLabel(plan.midtermExamMode),
					รูปแบบปลายภาค: examModeLabel(plan.finalExamMode),
					เวลากลางภาค: plan.midtermExamDurationMinutes ?? '',
					เวลาปลายภาค: plan.finalExamDurationMinutes ?? '',
					คะแนนย่อยไม่ลงตัว: plan.hasUnallocatedCategories ? 'ใช่' : 'ไม่ใช่'
				}));
			const worksheet = XLSX.utils.json_to_sheet(rows);
			const workbook = XLSX.utils.book_new();
			XLSX.utils.book_append_sheet(workbook, worksheet, kind === 'overview' ? 'Overview' : 'Exams');
			XLSX.writeFile(
				workbook,
				`โครงสร้างคะแนน-${kind === 'overview' ? 'ภาพรวม' : 'รูปแบบการสอบ'}.xlsx`
			);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถดาวน์โหลดเอกสารได้');
		} finally {
			exporting = false;
		}
	}

	onMount(initData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<PageShell
	title="โครงสร้างคะแนน"
	description="กำหนดคะแนนก่อนกลางภาค กลางภาค หลังกลางภาค ปลายภาค และรูปแบบการสอบของรายวิชาที่เปิดสอน"
>
	{#snippet actions()}
		{#if canManageSchoolAssessment}
			<div class="flex items-center gap-2 rounded-md border bg-background px-3 py-2">
				{#if settingsSaving}
					<Loader2 class="h-4 w-4 animate-spin text-muted-foreground" />
				{/if}
				<Switch
					id="teacher-assessment-access"
					checked={teacherAccessEnabled}
					onCheckedChange={(checked) => toggleTeacherAccess(checked === true)}
					disabled={settingsLoading || settingsSaving}
				/>
				<Label for="teacher-assessment-access" class="text-sm">เปิดให้ครูกรอก</Label>
			</div>
		{/if}
		<DropdownMenu.Root>
			<DropdownMenu.Trigger>
				<Button variant="outline" size="sm" disabled={exporting || plans.length === 0}>
					{#if exporting}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{:else}
						<Download class="mr-2 h-4 w-4" />
					{/if}
					ดาวน์โหลด
				</Button>
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end">
				<DropdownMenu.Item onclick={() => exportAssessmentReport('overview')}>
					<FileSpreadsheet class="mr-2 h-4 w-4" />
					ภาพรวมโครงสร้างคะแนน
				</DropdownMenu.Item>
				<DropdownMenu.Item onclick={() => exportAssessmentReport('exam')}>
					<ClipboardList class="mr-2 h-4 w-4" />
					รายวิชาที่มีการสอบ
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	{/snippet}

	{#if !canReadAssessment}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูโครงสร้างคะแนน"
			description="บัญชีนี้ยังไม่มีสิทธิ์ดูโครงสร้างคะแนนรายวิชาที่รับผิดชอบหรือภาพรวมทั้งโรงเรียน"
		/>
	{:else if loading}
		<PageSkeleton variant="table" rows={6} columns={7} />
	{:else if teacherAccessBlocked}
		<PageState
			variant="empty"
			title="ยังไม่เปิดให้ครูกรอกโครงสร้างคะแนน"
			description="ฝ่ายวิชาการยังปิดช่วงสำรวจโครงสร้างคะแนนรายวิชาอยู่"
		/>
	{:else}
		<div class="space-y-5">
			<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-6">
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">รายวิชา</p>
					<p class="mt-2 text-2xl font-semibold">{summary.total}</p>
				</div>
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">ร่าง/ยังไม่ตั้งค่า</p>
					<p class="mt-2 text-2xl font-semibold">{summary.draft}</p>
				</div>
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">บันทึกแล้ว</p>
					<p class="mt-2 text-2xl font-semibold">{summary.saved}</p>
				</div>
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">ส่งแล้ว</p>
					<p class="mt-2 text-2xl font-semibold">{summary.submitted}</p>
				</div>
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">สอบนอกตาราง</p>
					<p class="mt-2 text-2xl font-semibold">{summary.outside}</p>
				</div>
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">คะแนนย่อยไม่ลงตัว</p>
					<p class="mt-2 text-2xl font-semibold">{summary.unallocated}</p>
				</div>
			</div>

			<div class="rounded-lg border bg-background p-4">
				<div class="grid gap-4 lg:grid-cols-[1fr_1fr_1fr_160px_auto]">
					<div class="space-y-2">
						<Label>ปีการศึกษา</Label>
						<Select.Root type="single" value={selectedYearId} onValueChange={onYearChange}>
							<Select.Trigger>
								{structure.years.find((year) => year.id === selectedYearId)?.name ?? 'เลือกปี'}
							</Select.Trigger>
							<Select.Content>
								{#each structure.years as year (year.id)}
									<Select.Item value={year.id}>{year.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label>ภาคเรียน</Label>
						<Select.Root type="single" bind:value={selectedSemesterId}>
							<Select.Trigger>
								{filteredSemesters.find((semester) => semester.id === selectedSemesterId)?.name ??
									'ทุกภาคเรียน'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="">ทุกภาคเรียน</Select.Item>
								{#each filteredSemesters as semester (semester.id)}
									<Select.Item value={semester.id}
										>เทอม {semester.term} ({semester.name})</Select.Item
									>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label>ห้องเรียน</Label>
						<Select.Root type="single" bind:value={selectedClassroomId}>
							<Select.Trigger>
								{classrooms.find((classroom) => classroom.id === selectedClassroomId)?.name ??
									'ทุกห้องเรียน'}
							</Select.Trigger>
							<Select.Content class="max-h-[320px]">
								<Select.Item value="">ทุกห้องเรียน</Select.Item>
								{#each classrooms as classroom (classroom.id)}
									<Select.Item value={classroom.id}>{classroom.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label>สถานะ</Label>
						<Select.Root type="single" bind:value={selectedStatus}>
							<Select.Trigger
								>{statusOptions.find((option) => option.value === selectedStatus)
									?.label}</Select.Trigger
							>
							<Select.Content>
								{#each statusOptions as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="flex items-end">
						<Button class="w-full" onclick={loadPlans} disabled={loadingPlans}>
							{#if loadingPlans}
								<Loader2 class="mr-2 h-4 w-4 animate-spin" />
							{/if}
							ค้นหา
						</Button>
					</div>
				</div>
			</div>

			<div class="assessment-table-shell rounded-lg border bg-background">
				{#if canManageAssessment}
					<div class="assessment-table-toolbar flex justify-end border-b bg-background p-3">
						<Button
							class="w-full md:w-auto"
							variant="outline"
							size="sm"
							onclick={saveAllQuickScoreRows}
							disabled={!hasDirtyQuickScoreDrafts || savingAllQuickScores}
						>
							{#if savingAllQuickScores}
								<Loader2 class="mr-2 h-4 w-4 animate-spin" />
							{:else}
								<Save class="mr-2 h-4 w-4" />
							{/if}
							บันทึกการเปลี่ยนแปลง
						</Button>
					</div>
				{/if}
				<div
					class="assessment-table-scroll max-h-[calc(100vh-12rem)] overflow-auto md:max-h-[calc(100vh-16rem)] [&_[data-slot='table-container']]:overflow-visible"
				>
					<Table.Root class="min-w-[1240px]">
						<Table.Header>
							<Table.Row>
								<Table.Head
									class="assessment-sticky-head sticky top-0 z-30 bg-background"
									>รายวิชา</Table.Head
								>
								<Table.Head class="assessment-sticky-head sticky top-0 z-30 bg-background"
									>ห้องเรียนที่เปิด</Table.Head
								>
								<Table.Head class="assessment-sticky-head sticky top-0 z-30 bg-background"
									>ครูผู้สอน</Table.Head
								>
								{#each quickScoreColumns as column (column.field)}
									<Table.Head
										class="assessment-sticky-head sticky top-0 z-30 w-[78px] min-w-[78px] bg-background px-2 text-right"
										>{column.heading}</Table.Head
									>
								{/each}
								<Table.Head class="assessment-sticky-head sticky top-0 z-30 bg-background"
									>สอบกลางภาค</Table.Head
								>
								<Table.Head
									class="assessment-sticky-head sticky top-0 z-30 w-[84px] min-w-[84px] bg-background"
									>เวลากลางภาค</Table.Head
								>
								<Table.Head class="assessment-sticky-head sticky top-0 z-30 bg-background"
									>สอบปลายภาค</Table.Head
								>
								<Table.Head
									class="assessment-sticky-head sticky top-0 z-30 w-[84px] min-w-[84px] bg-background"
									>เวลาปลายภาค</Table.Head
								>
								<Table.Head
									class="assessment-sticky-head sticky top-0 z-30 bg-background px-2 text-right"
									>รวม</Table.Head
								>
								<Table.Head class="assessment-sticky-head sticky top-0 z-30 bg-background"
									>สถานะ</Table.Head
								>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#if loadingPlans}
								<Table.Row>
									<Table.Cell colspan={13} class="h-24 text-center text-muted-foreground">
										<Loader2 class="mx-auto mb-2 h-5 w-5 animate-spin" />
										กำลังโหลดข้อมูล
									</Table.Cell>
								</Table.Row>
							{:else if plans.length === 0}
								<Table.Row>
									<Table.Cell colspan={13}>
										<PageState
											variant="empty"
											title="ยังไม่มีรายวิชาตามตัวกรองนี้"
											description="เลือกปี ภาคเรียน หรือห้องเรียนอื่นเพื่อดูโครงสร้างคะแนน"
										/>
									</Table.Cell>
								</Table.Row>
							{:else}
								{#each plans as plan (assessmentPlanKey(plan))}
									{@const quickDraft = quickScoreDrafts[assessmentPlanKey(plan)]}
									{@const canEditPlan = !!quickDraft && canEditAssessmentPlan(plan)}
									<Table.Row>
										<Table.Cell>
											<div class="font-medium">{courseTitle(plan)}</div>
											<div class="text-xs text-muted-foreground">
												{plan.categoryCount} หมวด · {plan.itemCount} รายการย่อย
											</div>
										</Table.Cell>
										<Table.Cell>{classroomSummary(plan)}</Table.Cell>
										<Table.Cell>{plan.instructorName ?? '-'}</Table.Cell>
										{#each quickScoreColumns as column (column.field)}
											<Table.Cell class="assessment-score-cell w-[78px] min-w-[78px] px-2">
												{#if quickDraft}
													<Input
														type="number"
														min="0"
														step="0.5"
														class="assessment-score-input h-8 w-14 min-w-14 px-2 text-right tabular-nums [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
														aria-label={column.label}
														placeholder={column.heading}
														value={quickDraft[column.field] ?? ''}
														data-assessment-quick-score-input
														data-assessment-plan-key={assessmentPlanKey(plan)}
														data-assessment-field={column.field}
														required={canEditPlan}
														disabled={!canEditPlan || savingAllQuickScores}
														oninput={(event) =>
															setQuickScoreValue(plan, column.field, event.currentTarget.value)}
														onkeydown={handleQuickScoreEnter}
													/>
												{/if}
											</Table.Cell>
										{/each}
										<Table.Cell class="assessment-exam-cell">
											{#if quickDraft}
												<Select.Root
													type="single"
													value={quickDraft.midtermExamMode}
													onValueChange={(value) =>
														setQuickExamModeValue(plan, 'midtermExamMode', value)}
													disabled={!canEditPlan || savingAllQuickScores}
												>
													<Select.Trigger class="h-9 text-xs">
														{quickExamModeOptions.find(
															(option) => option.value === quickDraft.midtermExamMode
														)?.label}
													</Select.Trigger>
													<Select.Content>
														{#each quickExamModeOptions as option (option.value)}
															<Select.Item value={option.value}>{option.label}</Select.Item>
														{/each}
													</Select.Content>
												</Select.Root>
											{/if}
										</Table.Cell>
										<Table.Cell class="assessment-duration-cell w-[84px] min-w-[84px]">
											{#if quickDraft}
												<Input
													type="number"
													min="1"
													step="1"
													class="h-9 w-[72px] min-w-[72px] text-right tabular-nums [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
													aria-label="ระยะเวลากลางภาค"
													placeholder="นาที"
													value={quickDraft.midtermExamDurationMinutes ?? ''}
													data-assessment-plan-key={assessmentPlanKey(plan)}
													data-assessment-field="midtermExamDurationMinutes"
													required={canEditPlan && canEditExamDuration(quickDraft.midtermExamMode)}
													disabled={!canEditPlan ||
														savingAllQuickScores ||
														!canEditExamDuration(quickDraft.midtermExamMode)}
													oninput={(event) =>
														setQuickDurationValue(
															plan,
															'midtermExamDurationMinutes',
															event.currentTarget.value
														)}
												/>
											{/if}
										</Table.Cell>
										<Table.Cell class="assessment-exam-cell">
											{#if quickDraft}
												<Select.Root
													type="single"
													value={quickDraft.finalExamMode}
													onValueChange={(value) =>
														setQuickExamModeValue(plan, 'finalExamMode', value)}
													disabled={!canEditPlan || savingAllQuickScores}
												>
													<Select.Trigger class="h-9 text-xs">
														{quickExamModeOptions.find(
															(option) => option.value === quickDraft.finalExamMode
														)?.label}
													</Select.Trigger>
													<Select.Content>
														{#each quickExamModeOptions as option (option.value)}
															<Select.Item value={option.value}>{option.label}</Select.Item>
														{/each}
													</Select.Content>
												</Select.Root>
											{/if}
										</Table.Cell>
										<Table.Cell class="assessment-duration-cell w-[84px] min-w-[84px]">
											{#if quickDraft}
												<Input
													type="number"
													min="1"
													step="1"
													class="h-9 w-[72px] min-w-[72px] text-right tabular-nums [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
													aria-label="ระยะเวลาปลายภาค"
													placeholder="นาที"
													value={quickDraft.finalExamDurationMinutes ?? ''}
													data-assessment-plan-key={assessmentPlanKey(plan)}
													data-assessment-field="finalExamDurationMinutes"
													required={canEditPlan && canEditExamDuration(quickDraft.finalExamMode)}
													disabled={!canEditPlan ||
														savingAllQuickScores ||
														!canEditExamDuration(quickDraft.finalExamMode)}
													oninput={(event) =>
														setQuickDurationValue(
															plan,
															'finalExamDurationMinutes',
															event.currentTarget.value
														)}
												/>
											{/if}
										</Table.Cell>
										<Table.Cell class="px-2 text-right tabular-nums">
											{#if quickDraft}
												<div class={quickDraft.dirty ? 'font-semibold text-primary' : ''}>
													{quickScoreTotal(quickDraft)}
												</div>
											{:else}
												{plan.totalScore}
											{/if}
										</Table.Cell>
										<Table.Cell>
											<div class="flex flex-wrap items-center gap-2">
												<Badge variant={statusBadgeVariant(plan.status)}>
													{statusLabel(plan.status)}
												</Badge>
												{#if quickDraft?.dirty}
													<Badge variant="outline">ยังไม่บันทึก</Badge>
												{/if}
												{#if !plan.canManage}
													<Badge variant="outline">ดูอย่างเดียว</Badge>
												{/if}
												{#if plan.hasUnallocatedCategories}
													<Badge variant="destructive">
														<AlertTriangle class="h-3 w-3" />
														คะแนนย่อยไม่ลงตัว
													</Badge>
												{/if}
											</div>
										</Table.Cell>
									</Table.Row>
								{/each}
							{/if}
						</Table.Body>
					</Table.Root>
				</div>
			</div>
		</div>
	{/if}
</PageShell>
