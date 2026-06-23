<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		getAssessmentPlan,
		getAssessmentSettings,
		listAssessmentPlans,
		saveAssessmentPlan,
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
	type QuickExamModeField = 'midtermExamMode' | 'finalExamMode';
	type CoreCategoryCode = 'before_midterm' | 'midterm' | 'after_midterm' | 'final';
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
	const quickExamModeOptions: { value: QuickExamMode; label: string }[] = [
		{ value: 'none', label: 'ไม่มีสอบ' },
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
		submitted: plans.filter((plan) => plan.status === 'submitted').length,
		locked: plans.filter((plan) => plan.status === 'locked').length,
		outside: plans.reduce((total, plan) => total + plan.outsideTimetableCount, 0),
		unallocated: plans.filter((plan) => plan.hasUnallocatedCategories).length
	});

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

	function quickExamMode(value?: string | null): QuickExamMode {
		if (value === 'none' || value === 'outside_timetable') return value;
		return 'in_timetable';
	}

	function quickScoreDraftFromPlan(plan: AssessmentPlanSummary): QuickScoreDraft {
		return {
			beforeMidtermScore: plan.beforeMidtermScore,
			midtermScore: plan.midtermScore,
			afterMidtermScore: plan.afterMidtermScore,
			finalScore: plan.finalScore,
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
		if (field === 'midtermExamMode' && draft.midtermExamMode === 'none') {
			draft.midtermExamDurationMinutes = null;
		}
		if (field === 'finalExamMode' && draft.finalExamMode === 'none') {
			draft.finalExamDurationMinutes = null;
		}
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
			const examMode = template.examModeField ? draft[template.examModeField] : template.examMode;
			return {
				id: existing?.id,
				code: template.code,
				name: existing?.name || template.name,
				maxScore: quickScoreValue(draft[template.scoreField]),
				examMode,
				examDurationMinutes:
					template.durationField && examMode !== 'none'
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

	async function persistQuickScorePlan(plan: AssessmentPlanSummary) {
		if (!canManageAssessment || plan.status === 'locked') return false;
		const draft = quickDraftForPlan(plan);
		try {
			const detailResponse = await getAssessmentPlan(plan.classroomCourseId);
			await saveAssessmentPlan(
				plan.classroomCourseId,
				buildQuickScorePayload(detailResponse.data, draft)
			);
			draft.dirty = false;
			return true;
		} catch {
			return false;
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
				const saved = await persistQuickScorePlan(plan);
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
				บันทึกการเปลี่ยนแปลง
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

			<div class="overflow-x-auto rounded-lg border bg-background">
				<Table.Root class="min-w-[1160px]">
					<Table.Header>
						<Table.Row>
							<Table.Head>รายวิชา</Table.Head>
							<Table.Head>ห้องเรียนที่เปิด</Table.Head>
							<Table.Head>ครูผู้สอน</Table.Head>
							<Table.Head class="min-w-[280px]">คะแนน</Table.Head>
							<Table.Head class="min-w-[132px]">สอบกลาง</Table.Head>
							<Table.Head class="w-[104px]">เวลากลาง</Table.Head>
							<Table.Head class="min-w-[132px]">สอบปลาย</Table.Head>
							<Table.Head class="w-[104px]">เวลาปลาย</Table.Head>
							<Table.Head class="text-right">รวม</Table.Head>
							<Table.Head>สถานะ</Table.Head>
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
								{@const quickDraft = quickScoreDrafts[assessmentPlanKey(plan)]}
								{@const canEditPlan =
									canManageAssessment && plan.status !== 'locked' && !!quickDraft}
								<Table.Row>
									<Table.Cell>
										<div class="font-medium">{courseTitle(plan)}</div>
										<div class="text-xs text-muted-foreground">
											{plan.categoryCount} หมวด · {plan.itemCount} รายการย่อย
										</div>
									</Table.Cell>
									<Table.Cell>{classroomSummary(plan)}</Table.Cell>
									<Table.Cell>{plan.instructorName ?? '-'}</Table.Cell>
									<Table.Cell>
										{#if quickDraft}
											<div class="assessment-score-bundle-grid grid grid-cols-4 gap-1">
												<Input
													type="number"
													min="0"
													step="0.5"
													class="h-9 text-right tabular-nums"
													aria-label="ก่อนกลางภาค"
													placeholder="ก่อน"
													value={quickDraft.beforeMidtermScore ?? ''}
													disabled={!canEditPlan || savingAllQuickScores}
													oninput={(event) =>
														setQuickScoreValue(
															plan,
															'beforeMidtermScore',
															event.currentTarget.value
														)}
												/>
												<Input
													type="number"
													min="0"
													step="0.5"
													class="h-9 text-right tabular-nums"
													aria-label="กลางภาค"
													placeholder="กลาง"
													value={quickDraft.midtermScore ?? ''}
													disabled={!canEditPlan || savingAllQuickScores}
													oninput={(event) =>
														setQuickScoreValue(plan, 'midtermScore', event.currentTarget.value)}
												/>
												<Input
													type="number"
													min="0"
													step="0.5"
													class="h-9 text-right tabular-nums"
													aria-label="หลังกลางภาค"
													placeholder="หลัง"
													value={quickDraft.afterMidtermScore ?? ''}
													disabled={!canEditPlan || savingAllQuickScores}
													oninput={(event) =>
														setQuickScoreValue(
															plan,
															'afterMidtermScore',
															event.currentTarget.value
														)}
												/>
												<Input
													type="number"
													min="0"
													step="0.5"
													class="h-9 text-right tabular-nums"
													aria-label="ปลายภาค"
													placeholder="ปลาย"
													value={quickDraft.finalScore ?? ''}
													disabled={!canEditPlan || savingAllQuickScores}
													oninput={(event) =>
														setQuickScoreValue(plan, 'finalScore', event.currentTarget.value)}
												/>
											</div>
										{/if}
									</Table.Cell>
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
									<Table.Cell>
										{#if quickDraft}
											<Input
												type="number"
												min="1"
												step="1"
												class="h-9 text-right tabular-nums"
												aria-label="ระยะเวลากลางภาค"
												placeholder="นาที"
												value={quickDraft.midtermExamDurationMinutes ?? ''}
												disabled={!canEditPlan ||
													savingAllQuickScores ||
													quickDraft.midtermExamMode === 'none'}
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
									<Table.Cell>
										{#if quickDraft}
											<Input
												type="number"
												min="1"
												step="1"
												class="h-9 text-right tabular-nums"
												aria-label="ระยะเวลาปลายภาค"
												placeholder="นาที"
												value={quickDraft.finalExamDurationMinutes ?? ''}
												disabled={!canEditPlan ||
													savingAllQuickScores ||
													quickDraft.finalExamMode === 'none'}
												oninput={(event) =>
													setQuickDurationValue(
														plan,
														'finalExamDurationMinutes',
														event.currentTarget.value
													)}
											/>
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
								</Table.Row>
							{/each}
						{/if}
					</Table.Body>
				</Table.Root>
			</div>
		</div>
	{/if}
</PageShell>
