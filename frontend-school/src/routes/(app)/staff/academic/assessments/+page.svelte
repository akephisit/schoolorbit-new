<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		getAssessmentPlan,
		getAssessmentSettings,
		listAssessmentPlans,
		saveAssessmentPlan,
		submitAssessmentPlan,
		updateAssessmentSettings,
		type AssessmentCategory,
		type AssessmentExamMode,
		type AssessmentItem,
		type AssessmentPlanDetail,
		type AssessmentPlanStatus,
		type AssessmentPlanSummary,
		type SaveAssessmentCategoryRequest,
		type SaveAssessmentItemRequest
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
	import { Checkbox } from '$lib/components/ui/checkbox';
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
		ChevronDown,
		ChevronRight,
		ClipboardList,
		Download,
		FileSpreadsheet,
		Loader2,
		Plus,
		Save,
		Send,
		Trash2
	} from 'lucide-svelte';

	let { data } = $props();

	type StatusFilter = AssessmentPlanStatus | 'all';
	type EditorItem = {
		clientId: string;
		id?: string;
		name: string;
		maxScore: number;
		displayOrder: number;
		isActive: boolean;
	};
	type EditorCategory = Omit<SaveAssessmentCategoryRequest, 'items'> & {
		clientId: string;
		items: EditorItem[];
	};
	type ScheduledExamMode = Extract<AssessmentExamMode, 'in_timetable' | 'outside_timetable'>;
	type QuickScoreField = 'beforeMidtermScore' | 'midtermScore' | 'afterMidtermScore' | 'finalScore';
	type QuickDurationField = 'midtermExamDurationMinutes' | 'finalExamDurationMinutes';
	type QuickExamModeField = 'midtermExamMode' | 'finalExamMode';
	type CoreCategoryCode = 'before_midterm' | 'midterm' | 'after_midterm' | 'final';
	type QuickScoreDraft = {
		beforeMidtermScore: number | null;
		midtermScore: number | null;
		afterMidtermScore: number | null;
		finalScore: number | null;
		midtermExamMode: ScheduledExamMode;
		finalExamMode: ScheduledExamMode;
		midtermExamDurationMinutes: number | null;
		finalExamDurationMinutes: number | null;
		dirty: boolean;
		saving: boolean;
		submitting: boolean;
	};

	let teacherAccessEnabled = $state(true);

	const canReadAssessment = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_ASSESSMENT_READ_ASSIGNED,
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
		{ value: 'submitted', label: 'ส่งแล้ว' },
		{ value: 'locked', label: 'ล็อกแล้ว' }
	];
	const quickExamModeOptions: { value: ScheduledExamMode; label: string }[] = [
		{ value: 'in_timetable', label: 'ในตาราง' },
		{ value: 'outside_timetable', label: 'นอกตาราง' }
	];
	const coreAssessmentCategories: {
		code: CoreCategoryCode;
		name: string;
		displayOrder: number;
		scoreField: QuickScoreField;
		examMode: AssessmentExamMode;
		examModeField?: QuickExamModeField;
		durationField?: QuickDurationField;
	}[] = [
		{
			code: 'before_midterm',
			name: 'ก่อนกลางภาค',
			displayOrder: 10,
			scoreField: 'beforeMidtermScore',
			examMode: 'none'
		},
		{
			code: 'midterm',
			name: 'กลางภาค',
			displayOrder: 20,
			scoreField: 'midtermScore',
			examMode: 'in_timetable',
			examModeField: 'midtermExamMode',
			durationField: 'midtermExamDurationMinutes'
		},
		{
			code: 'after_midterm',
			name: 'หลังกลางภาค',
			displayOrder: 30,
			scoreField: 'afterMidtermScore',
			examMode: 'none'
		},
		{
			code: 'final',
			name: 'ปลายภาค',
			displayOrder: 40,
			scoreField: 'finalScore',
			examMode: 'in_timetable',
			examModeField: 'finalExamMode',
			durationField: 'finalExamDurationMinutes'
		}
	];
	const coreAssessmentCategoryCodes = new Set<string>(
		coreAssessmentCategories.map((category) => category.code)
	);

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
	let expandedPlanKey = $state<string | null>(null);
	let editorLoading = $state(false);
	let saving = $state(false);
	let submitting = $state(false);
	let savingAllQuickScores = $state(false);
	let editingCourse = $state<AssessmentPlanSummary | null>(null);
	let editingPlan = $state<AssessmentPlanDetail | null>(null);
	let editorCategories = $state<EditorCategory[]>([]);
	let quickScoreDrafts = $state<Record<string, QuickScoreDraft>>({});
	let localIdCounter = 0;

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
		submitted: plans.filter((plan) => plan.status === 'submitted').length,
		locked: plans.filter((plan) => plan.status === 'locked').length,
		outside: plans.reduce((total, plan) => total + plan.outsideTimetableCount, 0),
		unallocated: plans.filter((plan) => plan.hasUnallocatedCategories).length
	});

	function nextClientId(prefix: string) {
		localIdCounter += 1;
		return `${prefix}-${localIdCounter}`;
	}

	function statusLabel(status: AssessmentPlanStatus) {
		return statusOptions.find((option) => option.value === status)?.label ?? status;
	}

	function statusBadgeVariant(status: AssessmentPlanStatus) {
		if (status === 'submitted') return 'default';
		if (status === 'locked') return 'secondary';
		if (status === 'not_configured') return 'outline';
		return 'secondary';
	}

	function examModeLabel(mode: AssessmentExamMode | string) {
		return examModeOptions.find((option) => option.value === mode)?.label ?? mode;
	}

	function allocationStatusLabel(status: string) {
		if (status === 'complete') return 'ครบ';
		if (status === 'under_allocated') return 'ยังไม่ครบ';
		if (status === 'over_allocated') return 'เกิน';
		return 'ยังไม่เริ่ม';
	}

	function courseTitle(plan: AssessmentPlanSummary) {
		const subject = [plan.subjectCode, plan.subjectNameTh || plan.subjectNameEn]
			.filter(Boolean)
			.join(' ');
		return subject || 'รายวิชา';
	}

	function assessmentPlanKey(plan: AssessmentPlanSummary) {
		return `${plan.academicSemesterId}-${plan.subjectId}`;
	}

	function classroomSummary(plan: AssessmentPlanSummary) {
		if (!plan.classroomName) return '-';
		if (plan.classroomCount <= 1) return plan.classroomName;
		return `${plan.classroomName} (${plan.classroomCount} ห้อง)`;
	}

	function showsExamDuration(category: Pick<EditorCategory, 'examMode'>) {
		return category.examMode === 'in_timetable' || category.examMode === 'outside_timetable';
	}

	function setCategoryExamDuration(category: EditorCategory, value: string) {
		if (value.trim() === '') {
			category.examDurationMinutes = null;
			return;
		}
		const duration = Number.parseInt(value, 10);
		category.examDurationMinutes = Number.isNaN(duration) ? null : duration;
	}

	function examDurationLabel(duration?: number | null) {
		return duration ? `${duration} นาที` : 'ยังไม่ระบุเวลา';
	}

	function categoryTotal(category: EditorCategory) {
		return category.items
			.filter((item) => item.isActive)
			.reduce((total, item) => total + Number(item.maxScore || 0), 0);
	}

	function scheduledExamMode(value?: string | null): ScheduledExamMode {
		return value === 'outside_timetable' ? 'outside_timetable' : 'in_timetable';
	}

	function quickScoreDraftFromPlan(plan: AssessmentPlanSummary): QuickScoreDraft {
		return {
			beforeMidtermScore: plan.beforeMidtermScore,
			midtermScore: plan.midtermScore,
			afterMidtermScore: plan.afterMidtermScore,
			finalScore: plan.finalScore,
			midtermExamMode: scheduledExamMode(plan.midtermExamMode),
			finalExamMode: scheduledExamMode(plan.finalExamMode),
			midtermExamDurationMinutes: plan.midtermExamDurationMinutes ?? null,
			finalExamDurationMinutes: plan.finalExamDurationMinutes ?? null,
			dirty: false,
			saving: false,
			submitting: false
		};
	}

	function syncQuickScoreDrafts(nextPlans: AssessmentPlanSummary[]) {
		const nextDrafts: Record<string, QuickScoreDraft> = {};
		for (const plan of nextPlans) {
			const key = assessmentPlanKey(plan);
			const existing = quickScoreDrafts[key];
			nextDrafts[key] =
				existing && (existing.dirty || existing.saving || existing.submitting)
					? existing
					: quickScoreDraftFromPlan(plan);
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
		draft[field] = parseQuickNumber(value);
		draft.dirty = true;
	}

	function setQuickExamModeValue(
		plan: AssessmentPlanSummary,
		field: QuickExamModeField,
		value: string
	) {
		const draft = quickDraftForPlan(plan);
		draft[field] = scheduledExamMode(value);
		draft.dirty = true;
	}

	function quickScoreValue(value: number | null) {
		return Number(value ?? 0);
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

	function itemToSaveRequest(item: AssessmentItem): SaveAssessmentItemRequest {
		return {
			id: item.id,
			name: item.name,
			maxScore: item.maxScore,
			displayOrder: item.displayOrder,
			isActive: item.isActive
		};
	}

	function categoryToSaveRequest(category: AssessmentCategory): SaveAssessmentCategoryRequest {
		return {
			id: category.id,
			code: category.code,
			name: category.name,
			maxScore: category.maxScore,
			examMode: category.examMode,
			examDurationMinutes: category.examDurationMinutes ?? null,
			displayOrder: category.displayOrder,
			items: category.items.map(itemToSaveRequest)
		};
	}

	function buildQuickScorePayload(detail: AssessmentPlanDetail, draft: QuickScoreDraft) {
		const categoriesByCode = new Map<string, AssessmentCategory>();
		for (const category of detail.categories) {
			if (category.code) {
				categoriesByCode.set(category.code, category);
			}
		}

		const coreCategories = coreAssessmentCategories.map((template) => {
			const existing = categoriesByCode.get(template.code);
			return {
				id: existing?.id,
				code: template.code,
				name: existing?.name || template.name,
				maxScore: quickScoreValue(draft[template.scoreField]),
				examMode: template.examModeField ? draft[template.examModeField] : template.examMode,
				examDurationMinutes: template.durationField
					? quickDurationValue(draft[template.durationField])
					: null,
				displayOrder: existing?.displayOrder ?? template.displayOrder,
				items: existing?.items.map(itemToSaveRequest) ?? []
			};
		});

		const customCategories = detail.categories
			.filter((category) => !coreAssessmentCategoryCodes.has(category.code ?? ''))
			.map(categoryToSaveRequest);

		return {
			categories: [...coreCategories, ...customCategories]
		};
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

	function setEditorFromDetail(detail: AssessmentPlanDetail) {
		editingPlan = detail;
		editorCategories = detail.categories.map((category) => ({
			clientId: nextClientId('category'),
			id: category.id,
			code: category.code,
			name: category.name,
			maxScore: category.maxScore,
			examMode: category.examMode,
			examDurationMinutes: category.examDurationMinutes ?? null,
			displayOrder: category.displayOrder,
			items: category.items.map((item) => ({
				clientId: nextClientId('item'),
				id: item.id,
				name: item.name,
				maxScore: item.maxScore,
				displayOrder: item.displayOrder,
				isActive: item.isActive
			}))
		}));
	}

	function clearInlineEditor() {
		expandedPlanKey = null;
		editingCourse = null;
		editingPlan = null;
		editorCategories = [];
	}

	async function toggleInlineEditor(course: AssessmentPlanSummary) {
		if (!canManageAssessment || editorLoading || saving || submitting) return;
		const planKey = assessmentPlanKey(course);
		if (expandedPlanKey === planKey) {
			clearInlineEditor();
			return;
		}
		expandedPlanKey = planKey;
		editingCourse = course;
		editingPlan = null;
		editorCategories = [];
		editorLoading = true;
		try {
			const response = await getAssessmentPlan(course.classroomCourseId);
			setEditorFromDetail(response.data);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถเปิดโครงสร้างคะแนนได้');
			clearInlineEditor();
		} finally {
			editorLoading = false;
		}
	}

	function addCategory() {
		editorCategories.push({
			clientId: nextClientId('category'),
			code: 'custom',
			name: 'หมวดคะแนนใหม่',
			maxScore: 0,
			examMode: 'none',
			examDurationMinutes: null,
			displayOrder: (editorCategories.length + 1) * 10,
			items: []
		});
	}

	function removeCategory(index: number) {
		editorCategories.splice(index, 1);
		reorderCategories();
	}

	function addItem(categoryIndex: number) {
		const category = editorCategories[categoryIndex];
		category.items.push({
			clientId: nextClientId('item'),
			name: 'รายการคะแนนใหม่',
			maxScore: 0,
			displayOrder: (category.items.length + 1) * 10,
			isActive: true
		});
	}

	function removeItem(categoryIndex: number, itemIndex: number) {
		editorCategories[categoryIndex].items.splice(itemIndex, 1);
		editorCategories[categoryIndex].items.forEach((item, index) => {
			item.displayOrder = (index + 1) * 10;
		});
	}

	function reorderCategories() {
		editorCategories.forEach((category, index) => {
			category.displayOrder = (index + 1) * 10;
		});
	}

	function buildPayload() {
		reorderCategories();
		return {
			categories: editorCategories.map((category) => ({
				id: category.id,
				code: category.code,
				name: category.name.trim(),
				maxScore: Number(category.maxScore || 0),
				examMode: category.examMode,
				examDurationMinutes: showsExamDuration(category)
					? category.examDurationMinutes == null
						? null
						: Number(category.examDurationMinutes)
					: null,
				displayOrder: category.displayOrder,
				items: category.items.map((item, index) => ({
					id: item.id,
					name: item.name.trim(),
					maxScore: Number(item.maxScore || 0),
					displayOrder: (index + 1) * 10,
					isActive: item.isActive
				}))
			}))
		};
	}

	async function saveEditor() {
		if (!editingCourse) return;
		saving = true;
		try {
			const response = await saveAssessmentPlan(editingCourse.classroomCourseId, buildPayload());
			setEditorFromDetail(response.data);
			toast.success('บันทึกโครงสร้างคะแนนแล้ว');
			await loadPlans();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถบันทึกโครงสร้างคะแนนได้');
		} finally {
			saving = false;
		}
	}

	async function submitEditor() {
		if (!editingCourse) return;
		submitting = true;
		try {
			await saveAssessmentPlan(editingCourse.classroomCourseId, buildPayload());
			const response = await submitAssessmentPlan(editingCourse.classroomCourseId);
			setEditorFromDetail(response.data);
			toast.success('ส่งโครงสร้างคะแนนแล้ว');
			await loadPlans();
			clearInlineEditor();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถส่งโครงสร้างคะแนนได้');
		} finally {
			submitting = false;
		}
	}

	async function saveQuickScoreRow(
		plan: AssessmentPlanSummary,
		options: { reload?: boolean; silent?: boolean } = {}
	) {
		if (!canManageAssessment || plan.status === 'locked') return false;
		const draft = quickDraftForPlan(plan);
		draft.saving = true;
		try {
			const detailResponse = await getAssessmentPlan(plan.classroomCourseId);
			const saveResponse = await saveAssessmentPlan(
				plan.classroomCourseId,
				buildQuickScorePayload(detailResponse.data, draft)
			);
			if (expandedPlanKey === assessmentPlanKey(plan)) {
				setEditorFromDetail(saveResponse.data);
			}
			draft.dirty = false;
			if (!options.silent) {
				toast.success('บันทึกคะแนนรายวิชาแล้ว');
			}
			if (options.reload !== false) {
				await loadPlans();
			}
			return true;
		} catch (error) {
			if (!options.silent) {
				toast.error(error instanceof Error ? error.message : 'ไม่สามารถบันทึกคะแนนรายวิชาได้');
			}
			return false;
		} finally {
			draft.saving = false;
		}
	}

	async function submitQuickScoreRow(plan: AssessmentPlanSummary) {
		if (!canManageAssessment || plan.status === 'locked') return;
		const draft = quickDraftForPlan(plan);
		draft.submitting = true;
		try {
			const detailResponse = await getAssessmentPlan(plan.classroomCourseId);
			await saveAssessmentPlan(
				plan.classroomCourseId,
				buildQuickScorePayload(detailResponse.data, draft)
			);
			const submitResponse = await submitAssessmentPlan(plan.classroomCourseId);
			if (expandedPlanKey === assessmentPlanKey(plan)) {
				setEditorFromDetail(submitResponse.data);
			}
			draft.dirty = false;
			toast.success('ส่งโครงสร้างคะแนนแล้ว');
			await loadPlans();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถส่งโครงสร้างคะแนนได้');
		} finally {
			draft.submitting = false;
		}
	}

	async function saveAllQuickScoreRows() {
		if (!canManageAssessment || savingAllQuickScores) return;
		const dirtyPlans = plans.filter((plan) => quickScoreDrafts[assessmentPlanKey(plan)]?.dirty);
		if (dirtyPlans.length === 0) return;
		savingAllQuickScores = true;
		let savedCount = 0;
		try {
			for (const plan of dirtyPlans) {
				const saved = await saveQuickScoreRow(plan, { reload: false, silent: true });
				if (saved) savedCount += 1;
			}
			await loadPlans();
			if (savedCount === dirtyPlans.length) {
				toast.success('บันทึกคะแนนทั้งหมดแล้ว');
			} else {
				toast.error(`บันทึกสำเร็จ ${savedCount}/${dirtyPlans.length} รายวิชา`);
			}
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
			const rows = plans
				.filter(
					(plan) =>
						kind === 'overview' || plan.inTimetableCount > 0 || plan.outsideTimetableCount > 0
				)
				.map((plan) => ({
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
		{#if canManageAssessment}
			<Button
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
				บันทึกทั้งหมด
			</Button>
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
			<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">รายวิชา</p>
					<p class="mt-2 text-2xl font-semibold">{summary.total}</p>
				</div>
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">ร่าง/ยังไม่ตั้งค่า</p>
					<p class="mt-2 text-2xl font-semibold">{summary.draft}</p>
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

			<div class="overflow-hidden rounded-lg border bg-background">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head>รายวิชา</Table.Head>
							<Table.Head>ห้องเรียนที่เปิด</Table.Head>
							<Table.Head>ครูผู้สอน</Table.Head>
							<Table.Head class="min-w-[96px] text-right">ก่อน</Table.Head>
							<Table.Head class="min-w-[176px]">กลาง</Table.Head>
							<Table.Head class="min-w-[96px] text-right">หลัง</Table.Head>
							<Table.Head class="min-w-[176px]">ปลาย</Table.Head>
							<Table.Head class="text-right">รวม</Table.Head>
							<Table.Head>สถานะ</Table.Head>
							<Table.Head class="w-[152px] text-right">จัดการ</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#if loadingPlans}
							<Table.Row>
								<Table.Cell colspan={10} class="h-24 text-center text-muted-foreground">
									<Loader2 class="mx-auto mb-2 h-5 w-5 animate-spin" />
									กำลังโหลดข้อมูล
								</Table.Cell>
							</Table.Row>
						{:else if plans.length === 0}
							<Table.Row>
								<Table.Cell colspan={10}>
									<PageState
										variant="empty"
										title="ยังไม่มีรายวิชาตามตัวกรองนี้"
										description="เลือกปี ภาคเรียน หรือห้องเรียนอื่นเพื่อดูโครงสร้างคะแนน"
									/>
								</Table.Cell>
							</Table.Row>
						{:else}
							{#each plans as plan (assessmentPlanKey(plan))}
								{@const isExpanded = expandedPlanKey === assessmentPlanKey(plan)}
								{@const quickDraft = quickScoreDrafts[assessmentPlanKey(plan)]}
								{@const canEditPlan =
									canManageAssessment && plan.status !== 'locked' && !!quickDraft}
								{@const rowBusy =
									!!quickDraft &&
									(quickDraft.saving || quickDraft.submitting || savingAllQuickScores)}
								<Table.Row class={isExpanded ? 'bg-muted/20' : ''}>
									<Table.Cell>
										<div class="font-medium">{courseTitle(plan)}</div>
										<div class="text-xs text-muted-foreground">
											{plan.categoryCount} หมวด · {plan.itemCount} รายการย่อย
										</div>
									</Table.Cell>
									<Table.Cell>{classroomSummary(plan)}</Table.Cell>
									<Table.Cell>{plan.instructorName ?? '-'}</Table.Cell>
									<Table.Cell class="text-right">
										{#if quickDraft}
											<div class="assessment-quick-score-grid grid gap-1">
												<Input
													type="number"
													min="0"
													step="0.5"
													class="h-9 text-right tabular-nums"
													aria-label="ก่อนกลางภาค"
													value={quickDraft.beforeMidtermScore ?? ''}
													disabled={!canEditPlan || rowBusy}
													oninput={(event) =>
														setQuickScoreValue(
															plan,
															'beforeMidtermScore',
															event.currentTarget.value
														)}
												/>
											</div>
										{/if}
									</Table.Cell>
									<Table.Cell>
										{#if quickDraft}
											<div class="assessment-quick-score-grid grid grid-cols-[76px_80px] gap-2">
												<Input
													type="number"
													min="0"
													step="0.5"
													class="h-9 text-right tabular-nums"
													aria-label="กลางภาค"
													value={quickDraft.midtermScore ?? ''}
													disabled={!canEditPlan || rowBusy}
													oninput={(event) =>
														setQuickScoreValue(plan, 'midtermScore', event.currentTarget.value)}
												/>
												<Input
													type="number"
													min="1"
													step="1"
													class="h-9 text-right tabular-nums"
													aria-label="ระยะเวลากลางภาค"
													placeholder="นาที"
													value={quickDraft.midtermExamDurationMinutes ?? ''}
													disabled={!canEditPlan || rowBusy}
													oninput={(event) =>
														setQuickDurationValue(
															plan,
															'midtermExamDurationMinutes',
															event.currentTarget.value
														)}
												/>
												<div class="col-span-2">
													<Select.Root
														type="single"
														value={quickDraft.midtermExamMode}
														onValueChange={(value) =>
															setQuickExamModeValue(plan, 'midtermExamMode', value)}
														disabled={!canEditPlan || rowBusy}
													>
														<Select.Trigger class="h-8 text-xs">
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
												</div>
											</div>
										{/if}
									</Table.Cell>
									<Table.Cell class="text-right">
										{#if quickDraft}
											<div class="assessment-quick-score-grid grid gap-1">
												<Input
													type="number"
													min="0"
													step="0.5"
													class="h-9 text-right tabular-nums"
													aria-label="หลังกลางภาค"
													value={quickDraft.afterMidtermScore ?? ''}
													disabled={!canEditPlan || rowBusy}
													oninput={(event) =>
														setQuickScoreValue(
															plan,
															'afterMidtermScore',
															event.currentTarget.value
														)}
												/>
											</div>
										{/if}
									</Table.Cell>
									<Table.Cell>
										{#if quickDraft}
											<div class="assessment-quick-score-grid grid grid-cols-[76px_80px] gap-2">
												<Input
													type="number"
													min="0"
													step="0.5"
													class="h-9 text-right tabular-nums"
													aria-label="ปลายภาค"
													value={quickDraft.finalScore ?? ''}
													disabled={!canEditPlan || rowBusy}
													oninput={(event) =>
														setQuickScoreValue(plan, 'finalScore', event.currentTarget.value)}
												/>
												<Input
													type="number"
													min="1"
													step="1"
													class="h-9 text-right tabular-nums"
													aria-label="ระยะเวลาปลายภาค"
													placeholder="นาที"
													value={quickDraft.finalExamDurationMinutes ?? ''}
													disabled={!canEditPlan || rowBusy}
													oninput={(event) =>
														setQuickDurationValue(
															plan,
															'finalExamDurationMinutes',
															event.currentTarget.value
														)}
												/>
												<div class="col-span-2">
													<Select.Root
														type="single"
														value={quickDraft.finalExamMode}
														onValueChange={(value) =>
															setQuickExamModeValue(plan, 'finalExamMode', value)}
														disabled={!canEditPlan || rowBusy}
													>
														<Select.Trigger class="h-8 text-xs">
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
												</div>
											</div>
										{/if}
									</Table.Cell>
									<Table.Cell class="text-right tabular-nums">
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
											{#if plan.hasUnallocatedCategories}
												<Badge variant="destructive">
													<AlertTriangle class="h-3 w-3" />
													คะแนนย่อยไม่ลงตัว
												</Badge>
											{/if}
										</div>
									</Table.Cell>
									<Table.Cell class="text-right">
										<div class="flex justify-end gap-1">
											<Button
												variant={quickDraft?.dirty ? 'default' : 'outline'}
												size="icon"
												disabled={!canEditPlan || !quickDraft?.dirty || rowBusy}
												onclick={() => saveQuickScoreRow(plan)}
												title="บันทึกคะแนน"
											>
												{#if quickDraft?.saving}
													<Loader2 class="h-4 w-4 animate-spin" />
												{:else}
													<Save class="h-4 w-4" />
												{/if}
											</Button>
											<Button
												variant="ghost"
												size="icon"
												disabled={!canEditPlan || rowBusy}
												onclick={() => submitQuickScoreRow(plan)}
												title="ส่งโครงสร้าง"
											>
												{#if quickDraft?.submitting}
													<Loader2 class="h-4 w-4 animate-spin" />
												{:else}
													<Send class="h-4 w-4" />
												{/if}
											</Button>
											<Button
												variant="ghost"
												size="icon"
												disabled={!canManageAssessment || editorLoading || saving || submitting}
												onclick={() => toggleInlineEditor(plan)}
												title={isExpanded ? 'ปิดคะแนนย่อย' : 'คะแนนย่อย'}
												aria-expanded={isExpanded}
											>
												{#if isExpanded}
													<ChevronDown class="h-4 w-4" />
												{:else}
													<ChevronRight class="h-4 w-4" />
												{/if}
											</Button>
										</div>
									</Table.Cell>
								</Table.Row>
								{#if isExpanded}
									<Table.Row>
										<Table.Cell colspan={10} class="bg-muted/30 p-0">
											<div class="assessment-inline-editor-row space-y-4 border-t px-4 py-4">
												<div
													class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between"
												>
													<div class="min-w-0">
														<div class="flex flex-wrap items-center gap-2">
															<p class="font-medium">{courseTitle(plan)}</p>
															<Badge
																variant={statusBadgeVariant(editingPlan?.status ?? plan.status)}
															>
																{statusLabel(editingPlan?.status ?? plan.status)}
															</Badge>
														</div>
														<p class="mt-1 text-sm text-muted-foreground">
															{classroomSummary(plan)} · {plan.instructorName ?? '-'}
														</p>
													</div>
													<div class="flex flex-wrap gap-2">
														<Button
															variant="outline"
															size="sm"
															onclick={addCategory}
															disabled={editorLoading || saving || submitting}
														>
															<Plus class="mr-2 h-4 w-4" />
															เพิ่มหมวด
														</Button>
														<Button
															variant="ghost"
															size="sm"
															onclick={clearInlineEditor}
															disabled={editorLoading || saving || submitting}
														>
															<ChevronDown class="mr-2 h-4 w-4" />
															ปิด
														</Button>
														<Button
															variant="outline"
															size="sm"
															onclick={saveEditor}
															disabled={editorLoading || saving || submitting}
														>
															{#if saving}
																<Loader2 class="mr-2 h-4 w-4 animate-spin" />
															{:else}
																<Save class="mr-2 h-4 w-4" />
															{/if}
															บันทึกร่าง
														</Button>
														<Button
															size="sm"
															onclick={submitEditor}
															disabled={editorLoading || saving || submitting}
														>
															{#if submitting}
																<Loader2 class="mr-2 h-4 w-4 animate-spin" />
															{:else}
																<Send class="mr-2 h-4 w-4" />
															{/if}
															ส่งโครงสร้าง
														</Button>
													</div>
												</div>

												{#if editorLoading}
													<div
														class="rounded-md border bg-background px-4 py-8 text-center text-muted-foreground"
													>
														<Loader2 class="mx-auto mb-2 h-6 w-6 animate-spin" />
														กำลังโหลดโครงสร้างคะแนน
													</div>
												{:else if editorCategories.length === 0}
													<div
														class="rounded-md border bg-background px-4 py-8 text-center text-muted-foreground"
													>
														ยังไม่มีหมวดคะแนน
													</div>
												{:else}
													<div class="space-y-3">
														{#each editorCategories as category, categoryIndex (category.clientId)}
															<div class="rounded-lg border bg-background p-3 sm:p-4">
																<div
																	class="assessment-inline-category-grid grid gap-3 xl:grid-cols-[minmax(0,1.2fr)_120px_minmax(160px,220px)_150px_auto]"
																>
																	<div class="space-y-2">
																		<Label>หมวดคะแนน</Label>
																		<Input bind:value={category.name} />
																	</div>
																	<div class="space-y-2">
																		<Label>คะแนนเต็ม</Label>
																		<Input
																			type="number"
																			min="0"
																			step="0.5"
																			bind:value={category.maxScore}
																		/>
																	</div>
																	<div class="space-y-2">
																		<Label>รูปแบบ</Label>
																		<Select.Root type="single" bind:value={category.examMode}>
																			<Select.Trigger
																				>{examModeLabel(category.examMode)}</Select.Trigger
																			>
																			<Select.Content>
																				{#each examModeOptions as option (option.value)}
																					<Select.Item value={option.value}
																						>{option.label}</Select.Item
																					>
																				{/each}
																			</Select.Content>
																		</Select.Root>
																	</div>
																	<div class="space-y-2">
																		<Label>ระยะเวลาสอบ (นาที)</Label>
																		{#if showsExamDuration(category)}
																			<Input
																				type="number"
																				min="1"
																				step="1"
																				value={category.examDurationMinutes ?? ''}
																				oninput={(event) =>
																					setCategoryExamDuration(
																						category,
																						event.currentTarget.value
																					)}
																			/>
																		{:else}
																			<div
																				class="flex h-9 items-center rounded-md border bg-muted/40 px-3 text-sm text-muted-foreground"
																			>
																				-
																			</div>
																		{/if}
																	</div>
																	<div class="flex items-end justify-start xl:justify-end">
																		<Button
																			variant="ghost"
																			size="icon"
																			onclick={() => removeCategory(categoryIndex)}
																			disabled={saving || submitting}
																			title="ลบหมวด"
																		>
																			<Trash2 class="h-4 w-4" />
																		</Button>
																	</div>
																</div>

																<div class="mt-4 rounded-md bg-muted/40 p-3">
																	<div
																		class="mb-3 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
																	>
																		<div class="text-sm text-muted-foreground">
																			คะแนนย่อยรวม {categoryTotal(category)} / {Number(
																				category.maxScore || 0
																			)}
																			<span class="ml-2 font-medium">
																				{allocationStatusLabel(
																					category.items.length === 0
																						? Number(category.maxScore || 0) === 0
																							? 'not_started'
																							: 'complete'
																						: Math.abs(
																									categoryTotal(category) -
																										Number(category.maxScore || 0)
																							  ) < 0.0001
																							? 'complete'
																							: categoryTotal(category) <
																								  Number(category.maxScore || 0)
																								? 'under_allocated'
																								: 'over_allocated'
																				)}
																			</span>
																		</div>
																		<Button
																			variant="outline"
																			size="sm"
																			onclick={() => addItem(categoryIndex)}
																			disabled={saving || submitting}
																		>
																			<Plus class="mr-2 h-4 w-4" />
																			เพิ่มคะแนนย่อย
																		</Button>
																	</div>

																	{#if category.items.length === 0}
																		<p class="text-sm text-muted-foreground">
																			ยังไม่แยกคะแนนย่อย ระบบจะถือว่าหมวดนี้เป็นรายการเดียวชั่วคราว
																		</p>
																	{:else}
																		<div class="space-y-2">
																			{#each category.items as item, itemIndex (item.clientId)}
																				<div
																					class="assessment-inline-item-grid grid gap-3 rounded-md border bg-background p-3 md:grid-cols-[minmax(0,1fr)_120px_110px_auto]"
																				>
																					<div class="space-y-2">
																						<Label>รายการ</Label>
																						<Input bind:value={item.name} />
																					</div>
																					<div class="space-y-2">
																						<Label>คะแนน</Label>
																						<Input
																							type="number"
																							min="0"
																							step="0.5"
																							bind:value={item.maxScore}
																						/>
																					</div>
																					<label
																						class="flex min-h-10 items-end gap-2 text-sm md:pb-2"
																					>
																						<Checkbox bind:checked={item.isActive} />
																						ใช้งาน
																					</label>
																					<div class="flex items-end justify-start md:justify-end">
																						<Button
																							variant="ghost"
																							size="icon"
																							onclick={() => removeItem(categoryIndex, itemIndex)}
																							disabled={saving || submitting}
																							title="ลบคะแนนย่อย"
																						>
																							<Trash2 class="h-4 w-4" />
																						</Button>
																					</div>
																				</div>
																			{/each}
																		</div>
																	{/if}
																</div>
															</div>
														{/each}
													</div>
												{/if}
											</div>
										</Table.Cell>
									</Table.Row>
								{/if}
							{/each}
						{/if}
					</Table.Body>
				</Table.Root>
			</div>
		</div>
	{/if}
</PageShell>
