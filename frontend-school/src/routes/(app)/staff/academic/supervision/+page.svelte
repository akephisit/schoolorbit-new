<script lang="ts">
	import { onMount } from 'svelte';
	import {
		BarChart3,
		BookOpenCheck,
		Check,
		ChevronsUpDown,
		ArrowDown,
		ArrowUp,
		Eye,
		FileSignature,
		Loader2,
		Plus,
		RefreshCw,
		Send,
		Settings2,
		Trash2,
		UserCheck
	} from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import {
		getAcademicStructure,
		getSchoolDays,
		type AcademicStructureData
	} from '$lib/api/academic';
	import { lookupStaff, type StaffLookupItem } from '$lib/api/lookup';
	import { getMyTimetable, type TimetableEntry } from '$lib/api/timetable';
	import {
		acknowledgeSupervisionObservation,
		approveSupervisionObservation,
		approveSupervisionObservationRequest,
		createSupervisionCycle,
		createSupervisionTemplate,
		getSupervisionCycleProgress,
		listSupervisionCycles,
		listSupervisionObservations,
		listSupervisionTemplates,
		publishSupervisionObservation,
		requestSupervisionObservation,
		returnSupervisionObservation,
		returnSupervisionObservationRequest,
		saveMySupervisionEvaluation,
		submitMySupervisionEvaluation,
		submitSupervisionObservationForReview,
		updateSupervisionCycle,
		updateSupervisionTemplate,
		type CreateSupervisionCycleRequest,
		type CreateSupervisionTemplateRequest,
		type SaveEvaluationRequest,
		type SupervisionCycle,
		type SupervisionCycleStatus,
		type SupervisionCycleProgress,
		type SupervisionObservation,
		type SupervisionObservationStatus,
		type SupervisionTemplate,
		type SupervisionTemplateStatus
	} from '$lib/api/supervision';
	import {
		calculateRubricDraftSummary,
		createBlankRubricItem,
		createBlankRubricSection,
		createPaperSupervisionRubricSections,
		sectionRubricProgress,
		type RubricFormSection,
		type RubricItemType,
		type RubricResponseDraft
	} from '$lib/utils/supervision-rubric';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import { cn } from '$lib/utils';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import * as Alert from '$lib/components/ui/alert';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Command from '$lib/components/ui/command';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Popover from '$lib/components/ui/popover';
	import { Progress } from '$lib/components/ui/progress';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import * as Tabs from '$lib/components/ui/tabs';
	import { Textarea } from '$lib/components/ui/textarea';

	type ResponseDraft = RubricResponseDraft;
	type TemplateFormState = {
		title: string;
		description: string;
		status: SupervisionTemplateStatus;
		ratingMin: number;
		ratingMax: number;
		sections: RubricFormSection[];
	};

	const timetableGridDays = getSchoolDays();

	type TimetablePeriodRow = {
		key: string;
		label: string;
		timeLabel: string;
		sort: number;
	};
	type ObservationDetailItem = {
		label: string;
		value: string;
	};
	type CycleFormState = {
		academicYear: number;
		semester: string;
		academicSemesterId: string;
		title: string;
		description: string;
		templateId: string;
		status: SupervisionCycleStatus;
		bookingOpensDate: string;
		bookingOpensTime: string;
		bookingClosesDate: string;
		bookingClosesTime: string;
		startsDate: string;
		startsTime: string;
		endsDate: string;
		endsTime: string;
	};

	const cycleStatusCreateOptions: {
		value: SupervisionCycleStatus;
		label: string;
		description: string;
	}[] = [
		{
			value: 'open',
			label: 'เปิดให้จองทันที',
			description: 'ครูที่มีสิทธิ์จองจะเห็นรอบนี้ในหน้าขอรับนิเทศ'
		},
		{
			value: 'draft',
			label: 'บันทึกเป็นร่าง',
			description: 'เตรียมข้อมูลไว้ก่อน ยังไม่แสดงในหน้าจองของครู'
		},
		{
			value: 'closed',
			label: 'ปิดการจอง',
			description: 'ใช้เมื่อต้องการสร้างรอบไว้แต่ยังไม่รับคำขอ'
		}
	];

	function createDefaultTemplateForm(): TemplateFormState {
		return {
			title: 'แบบนิเทศการจัดการเรียนรู้',
			description: 'แบบประเมินการจัดการเรียนรู้ตามหัวข้อการนิเทศในชั้นเรียน',
			status: 'draft',
			ratingMin: 1,
			ratingMax: 5,
			sections: createPaperSupervisionRubricSections()
		};
	}

	function templateSectionsToRubricForm(template: SupervisionTemplate | null): RubricFormSection[] {
		if (!template) return [];
		return template.sections.map((section, sectionIndex) => ({
			localId: section.id,
			title: section.title,
			description: section.description ?? '',
			sortOrder: section.sortOrder || sectionIndex + 1,
			items: section.items.map((item, itemIndex) => ({
				localId: item.id,
				label: item.label,
				description: item.description ?? '',
				itemType: item.itemType,
				required: item.required,
				sortOrder: item.sortOrder || itemIndex + 1
			}))
		}));
	}

	let { data } = $props();

	let loading = $state(true);
	let loadingTimetable = $state(false);
	let saving = $state(false);
	let savingAction = $state<string | null>(null);
	let savingTemplate = $state(false);
	let savingEvaluation = $state<'draft' | 'submit' | null>(null);
	let evaluationDialogOpen = $state(false);
	let activeTab = $state('mine');
	let cycles = $state<SupervisionCycle[]>([]);
	let templates = $state<SupervisionTemplate[]>([]);
	let observations = $state<SupervisionObservation[]>([]);
	let timetableEntries = $state<TimetableEntry[]>([]);
	let staffList = $state<StaffLookupItem[]>([]);
	let academicStructure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let selectedCycleId = $state('');
	let selectedTimetableEntryId = $state('');
	let selectedBookingDate = $state('');
	let bookingWeekStartDate = $state('');
	let bookingWeekCycleId = $state('');
	let manualMode = $state(false);
	let manualLessonDate = $state('');
	let manualLessonTime = $state('08:30');
	let manualLesson = $state({
		subjectName: '',
		classroomLabel: '',
		roomLabel: '',
		periodLabel: '',
		reason: ''
	});
	let requestEvaluatorIds = $state<Record<string, string[]>>({});
	let requestReturnComments = $state<Record<string, string>>({});
	let evaluatorPickerOpenByRequest = $state<Record<string, boolean>>({});
	let evaluationObservationId = $state('');
	let responseDrafts = $state<{ [itemId: string]: ResponseDraft }>({});
	let acknowledgeComment = $state('');
	let reviewComment = $state('');
	let progressCycleId = $state('');
	let progress = $state<SupervisionCycleProgress | null>(null);
	let createCycleDialogOpen = $state(false);
	let createTemplateDialogOpen = $state(false);
	let previewTemplateDialogOpen = $state(false);
	let previewTemplateId = $state('');
	let editingTemplateId = $state('');
	let cycleAcademicYearId = $state('');
	let loadedTimetableCycleId = $state('');
	let cycleForm = $state<CycleFormState>({
		academicYear: 0,
		semester: '',
		academicSemesterId: '',
		title: '',
		description: '',
		templateId: '',
		status: 'open',
		bookingOpensDate: '',
		bookingOpensTime: '08:00',
		bookingClosesDate: '',
		bookingClosesTime: '16:30',
		startsDate: '',
		startsTime: '08:00',
		endsDate: '',
		endsTime: '16:30'
	});
	let templateForm = $state<TemplateFormState>(createDefaultTemplateForm());

	const currentUserId = $derived($authStore.user?.id ?? '');
	const mutationBusy = $derived(
		saving || savingAction !== null || savingTemplate || savingEvaluation !== null
	);
	const canRequest = $derived($can.has(PERMISSIONS.SUPERVISION_REQUEST_OWN));
	const canManageSchool = $derived($can.has(PERMISSIONS.SUPERVISION_MANAGE_SCHOOL));
	const canManageRequests = $derived(
		$can.hasAny(
			PERMISSIONS.SUPERVISION_MANAGE_SCHOOL,
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_TREE
		)
	);
	const canEvaluate = $derived($can.has(PERMISSIONS.SUPERVISION_EVALUATE_ASSIGNED));
	const canApprove = $derived($can.has(PERMISSIONS.SUPERVISION_APPROVE_SCHOOL));
	const canReadObservations = $derived(
		$can.hasAny(
			PERMISSIONS.SUPERVISION_READ_OWN,
			PERMISSIONS.SUPERVISION_READ_ASSIGNED,
			PERMISSIONS.SUPERVISION_READ_ORGANIZATION_UNIT,
			PERMISSIONS.SUPERVISION_READ_ORGANIZATION_TREE,
			PERMISSIONS.SUPERVISION_READ_SCHOOL
		) ||
			canManageRequests ||
			canApprove
	);
	const canReport = $derived(
		$can.has(PERMISSIONS.SUPERVISION_READ_SCHOOL) || canManageSchool || canApprove
	);
	const activeAcademicYear = $derived(
		academicStructure.years.find((year) => year.is_active) ?? academicStructure.years[0] ?? null
	);
	const cycleYear = $derived(
		academicStructure.years.find((year) => year.id === cycleAcademicYearId) ?? activeAcademicYear
	);
	const cycleSemesters = $derived(
		academicStructure.semesters.filter(
			(semester) => semester.academic_year_id === (cycleYear?.id ?? '')
		)
	);
	const openCycles = $derived(cycles.filter((cycle) => cycle.status === 'open'));
	const activeBookingCycles = $derived(openCycles.filter((cycle) => cycleAcceptsBookings(cycle)));
	const currentBookingCycle = $derived(
		activeBookingCycles.find((cycle) => cycle.id === selectedCycleId) ??
			activeBookingCycles[0] ??
			null
	);
	const requestedObservations = $derived(
		observations.filter((observation) => observation.status === 'requested')
	);
	const selectedCycleDetail = $derived(
		cycles.find((cycle) => cycle.id === selectedCycleId) ?? null
	);
	const selectedCycleSemester = $derived(
		selectedCycleDetail?.academicSemesterId
			? (academicStructure.semesters.find(
					(semester) => semester.id === selectedCycleDetail.academicSemesterId
				) ?? null)
			: null
	);
	const selectedCycleAcademicYear = $derived(
		selectedCycleSemester
			? (academicStructure.years.find(
					(year) => year.id === selectedCycleSemester.academic_year_id
				) ?? null)
			: selectedCycleDetail
				? (academicStructure.years.find((year) => year.year === selectedCycleDetail.academicYear) ??
					activeAcademicYear)
				: activeAcademicYear
	);
	const timetableSchoolDays = $derived(
		selectedCycleAcademicYear
			? getSchoolDays(selectedCycleAcademicYear.school_days)
			: timetableGridDays
	);
	const bookingWeekDays = $derived(
		timetableSchoolDays.map((day) => ({
			...day,
			date: dateForTimetableDay(day.value)
		}))
	);
	const selectedTimetableEntry = $derived(
		timetableEntries.find((entry) => entry.id === selectedTimetableEntryId) ?? null
	);
	const myObservations = $derived(
		observations.filter((observation) => observation.observedUserId === currentUserId)
	);
	const assignedObservations = $derived(
		observations.filter((observation) =>
			observation.evaluators.some((evaluator) => evaluator.evaluatorUserId === currentUserId)
		)
	);
	const activeAssignedObservations = $derived(
		assignedObservations.filter(
			(observation) => currentUserEvaluator(observation)?.status !== 'submitted'
		)
	);
	const submittedAssignedObservations = $derived(
		assignedObservations.filter(
			(observation) => currentUserEvaluator(observation)?.status === 'submitted'
		)
	);
	const selectedEvaluation = $derived(
		observations.find((observation) => observation.id === evaluationObservationId) ?? null
	);
	const selectedEvaluationTemplate = $derived(
		selectedEvaluation
			? (templates.find((template) => template.id === selectedEvaluation.templateId) ?? null)
			: null
	);
	const previewTemplate = $derived(
		templates.find((template) => template.id === previewTemplateId) ?? null
	);
	const selectedEvaluationRubricSections = $derived(
		templateSectionsToRubricForm(selectedEvaluationTemplate)
	);
	const selectedEvaluationDraftSummary = $derived(
		calculateRubricDraftSummary(
			selectedEvaluationRubricSections,
			responseDrafts,
			selectedEvaluationTemplate?.ratingMax ?? 5
		)
	);
	const reviewableObservations = $derived(
		observations.filter(
			(item) =>
				item.status === 'evaluators_submitted' ||
				item.status === 'under_review' ||
				item.status === 'approved'
		)
	);
	const progressPercent = $derived(
		progress && progress.totalObservations > 0
			? Math.round((progress.completedCount / progress.totalObservations) * 100)
			: 0
	);

	$effect(() => {
		if (!cycleAcademicYearId && activeAcademicYear) {
			cycleAcademicYearId = activeAcademicYear.id;
		}
	});

	$effect(() => {
		if (!cycleYear) return;
		const semesters = academicStructure.semesters.filter(
			(semester) => semester.academic_year_id === cycleYear.id
		);
		if (semesters.length === 0) return;
		if (
			!cycleForm.academicSemesterId ||
			!semesters.some((s) => s.id === cycleForm.academicSemesterId)
		) {
			const selectedSemester = semesters.find((semester) => semester.is_active) ?? semesters[0];
			setCycleSemester(selectedSemester.id);
		}
	});

	$effect(() => {
		if (!cycleForm.academicSemesterId) return;
		const semester = academicStructure.semesters.find(
			(item) => item.id === cycleForm.academicSemesterId
		);
		const year = semester
			? academicStructure.years.find((item) => item.id === semester.academic_year_id)
			: null;
		if (!semester || !year) return;
		if (cycleForm.academicYear !== year.year || cycleForm.semester !== semester.term) {
			setCycleSemester(semester.id);
		}
	});

	$effect(() => {
		if (!selectedCycleId) return;
		void refreshTimetableForCycle(selectedCycleId);
	});

	$effect(() => {
		if (!currentBookingCycle) return;
		if (selectedCycleId !== currentBookingCycle.id) {
			selectedCycleId = currentBookingCycle.id;
		}
		if (bookingWeekCycleId !== currentBookingCycle.id) {
			bookingWeekCycleId = currentBookingCycle.id;
			bookingWeekStartDate = defaultBookingWeekStartDate(currentBookingCycle);
			selectedTimetableEntryId = '';
			selectedBookingDate = '';
		}
	});

	$effect(() => {
		if (!selectedTimetableEntryId) return;
		if (
			!timetableEntriesForSelectedCycle().some((entry) => entry.id === selectedTimetableEntryId)
		) {
			selectedTimetableEntryId = '';
			selectedBookingDate = '';
		}
	});

	function setCycleSemester(semesterId: string) {
		const semester = academicStructure.semesters.find((item) => item.id === semesterId);
		if (!semester) return;
		const year = academicStructure.years.find((item) => item.id === semester.academic_year_id);
		if (!year) return;

		cycleAcademicYearId = year.id;
		cycleForm.academicYear = year.year;
		cycleForm.semester = semester.term;
		cycleForm.academicSemesterId = semester.id;
		cycleForm.startsDate ||= semester.start_date;
		cycleForm.endsDate ||= semester.end_date;
		cycleForm.bookingOpensDate ||= semester.start_date;
		cycleForm.bookingClosesDate ||= semester.end_date;
	}

	function formatDate(value?: string | null): string {
		if (!value) return '-';
		return new Intl.DateTimeFormat('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short'
		}).format(new Date(value));
	}

	function semesterLabel(semesterId?: string | null): string {
		if (!semesterId) return 'ไม่ผูกภาคเรียน';
		const semester = academicStructure.semesters.find((item) => item.id === semesterId);
		const year = semester
			? academicStructure.years.find((item) => item.id === semester.academic_year_id)
			: null;
		if (!semester) return 'ไม่พบภาคเรียน';
		return `${semester.name || `ภาคเรียนที่ ${semester.term}`} ${year?.name ?? ''}`.trim();
	}

	function cycleLabel(cycle: SupervisionCycle): string {
		const period = cycle.academicSemesterId
			? semesterLabel(cycle.academicSemesterId)
			: `ปี ${cycle.academicYear} / ภาคเรียน ${cycle.semester}`;
		return `${cycle.title} - ${period}`;
	}

	function statusLabel(status: SupervisionObservationStatus | SupervisionCycle['status']): string {
		const labels: Partial<
			Record<SupervisionObservationStatus | SupervisionCycle['status'], string>
		> = {
			draft: 'ร่าง',
			open: 'เปิด',
			closed: 'ปิด',
			archived: 'เก็บถาวร',
			requested: 'รออนุมัติ',
			planned: 'นัดหมายแล้ว',
			in_progress: 'กำลังประเมิน',
			evaluators_submitted: 'ผู้ประเมินส่งครบ',
			under_review: 'รอตรวจทาน',
			returned: 'ส่งกลับ',
			approved: 'อนุมัติแล้ว',
			published: 'เผยแพร่แล้ว',
			acknowledged: 'รับทราบแล้ว',
			completed: 'เสร็จสิ้น',
			cancelled: 'ยกเลิก'
		};
		return labels[status] ?? status;
	}

	function templateStatusLabel(status: SupervisionTemplateStatus): string {
		const labels: Record<SupervisionTemplateStatus, string> = {
			draft: 'ร่าง',
			active: 'ใช้งาน',
			archived: 'เก็บถาวร'
		};
		return labels[status] ?? status;
	}

	function templateItemCount(template: SupervisionTemplate): number {
		return template.sections.reduce((sum, section) => sum + section.items.length, 0);
	}

	function templateRatingColumns(template: SupervisionTemplate): number[] {
		const min = Math.min(Number(template.ratingMin), Number(template.ratingMax));
		const max = Math.max(Number(template.ratingMin), Number(template.ratingMax));
		return Array.from({ length: max - min + 1 }, (_, index) => max - index);
	}

	function openTemplatePreviewDialog(template: SupervisionTemplate) {
		previewTemplateId = template.id;
		previewTemplateDialogOpen = true;
	}

	function setTemplatePreviewDialogOpen(open: boolean) {
		previewTemplateDialogOpen = open;
		if (!open) {
			previewTemplateId = '';
		}
	}

	function toLocalDateInputValue(date: Date): string {
		const year = date.getFullYear();
		const month = String(date.getMonth() + 1).padStart(2, '0');
		const day = String(date.getDate()).padStart(2, '0');
		return `${year}-${month}-${day}`;
	}

	function parseLocalDate(date: string): Date {
		const [year = '1970', month = '1', day = '1'] = date.split('-');
		return new Date(Number(year), Number(month) - 1, Number(day));
	}

	function addDays(date: Date, days: number): Date {
		return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
	}

	function startOfWeek(date: Date): Date {
		const mondayOffset = (date.getDay() + 6) % 7;
		return addDays(date, -mondayOffset);
	}

	function addWeeks(date: string, weeks: number): string {
		return toLocalDateInputValue(addDays(parseLocalDate(date), weeks * 7));
	}

	function dateForTimetableDay(day: string): string {
		const offsets: Record<string, number> = {
			MON: 0,
			TUE: 1,
			WED: 2,
			THU: 3,
			FRI: 4,
			SAT: 5,
			SUN: 6
		};
		const weekStart = bookingWeekStartDate || toLocalDateInputValue(startOfWeek(new Date()));
		return toLocalDateInputValue(addDays(parseLocalDate(weekStart), offsets[day] ?? 0));
	}

	function formatShortDate(date: string): string {
		return new Intl.DateTimeFormat('th-TH', {
			day: 'numeric',
			month: 'short'
		}).format(parseLocalDate(date));
	}

	function bookingWeekLabel(): string {
		if (!bookingWeekStartDate) return 'สัปดาห์ปัจจุบัน';
		const start = parseLocalDate(bookingWeekStartDate);
		const end = addDays(start, 6);
		return `${formatShortDate(toLocalDateInputValue(start))} - ${formatShortDate(
			toLocalDateInputValue(end)
		)}`;
	}

	function cycleDateRange(cycle: SupervisionCycle): { start: string; end: string } {
		return {
			start: toLocalDateInputValue(new Date(cycle.startsAt)),
			end: toLocalDateInputValue(new Date(cycle.endsAt))
		};
	}

	function cycleAcceptsBookings(cycle: SupervisionCycle): boolean {
		const now = new Date();
		if (cycle.status !== 'open') return false;
		if (cycle.bookingOpensAt && now < new Date(cycle.bookingOpensAt)) return false;
		if (cycle.bookingClosesAt && now > new Date(cycle.bookingClosesAt)) return false;
		return now >= new Date(cycle.startsAt) && now <= new Date(cycle.endsAt);
	}

	function defaultBookingWeekStartDate(cycle: SupervisionCycle): string {
		const now = new Date();
		const cycleStart = new Date(cycle.startsAt);
		const cycleEnd = new Date(cycle.endsAt);
		const base = now >= cycleStart && now <= cycleEnd ? now : cycleStart;
		return toLocalDateInputValue(startOfWeek(base));
	}

	function bookingDateInCycle(date: string, cycle: SupervisionCycle | null = currentBookingCycle) {
		if (!cycle) return false;
		const range = cycleDateRange(cycle);
		return date >= range.start && date <= range.end;
	}

	function canNavigateBookingWeek(direction: -1 | 1): boolean {
		if (!currentBookingCycle || !bookingWeekStartDate) return false;
		const nextStart = addWeeks(bookingWeekStartDate, direction);
		const nextEnd = toLocalDateInputValue(addDays(parseLocalDate(nextStart), 6));
		const cycleRange = cycleDateRange(currentBookingCycle);
		return nextEnd >= cycleRange.start && nextStart <= cycleRange.end;
	}

	function goToPreviousBookingWeek() {
		if (!canNavigateBookingWeek(-1)) return;
		bookingWeekStartDate = addWeeks(bookingWeekStartDate, -1);
		selectedTimetableEntryId = '';
		selectedBookingDate = '';
	}

	function goToNextBookingWeek() {
		if (!canNavigateBookingWeek(1)) return;
		bookingWeekStartDate = addWeeks(bookingWeekStartDate, 1);
		selectedTimetableEntryId = '';
		selectedBookingDate = '';
	}

	function resetToCurrentBookingWeek() {
		if (!currentBookingCycle) return;
		bookingWeekStartDate = defaultBookingWeekStartDate(currentBookingCycle);
		selectedTimetableEntryId = '';
		selectedBookingDate = '';
	}

	function timetableLabel(entry: TimetableEntry): string {
		const title = entry.subject_name_th || entry.title || entry.subject_code || 'คาบสอน';
		const period = entry.period_name ? ` ${entry.period_name}` : '';
		const room = entry.room_code ? ` ห้อง ${entry.room_code}` : '';
		const classroom = entry.classroom_name ? ` ${entry.classroom_name}` : '';
		return `${entry.day_of_week}${period} - ${title}${classroom}${room}`;
	}

	function timetableObservedAt(entry: TimetableEntry, bookingDate: string): string {
		const startTime = entry.start_time?.slice(0, 5) || '08:00';
		return combineLocalDateTime(bookingDate, startTime);
	}

	function observationDate(observation: SupervisionObservation): string | null {
		const observedAt =
			observation.observedAt ??
			observation.lessonSnapshot.observedAt ??
			observation.manualLesson?.observedAt;
		return observedAt ? toLocalDateInputValue(new Date(observedAt)) : null;
	}

	function observationForTimetableCell(
		entry: TimetableEntry,
		bookingDate: string
	): SupervisionObservation | null {
		const matches = observations
			.filter(
				(observation) =>
					observation.cycleId === currentBookingCycle?.id &&
					observation.observedUserId === currentUserId &&
					observation.timetableEntryId === entry.id &&
					observation.status !== 'cancelled' &&
					observationDate(observation) === bookingDate
			)
			.sort((left, right) => Date.parse(right.updatedAt) - Date.parse(left.updatedAt));
		return matches[0] ?? null;
	}

	function timetableEntryTitle(entry: TimetableEntry): string {
		return entry.subject_name_th || entry.title || entry.subject_code || 'คาบสอน';
	}

	function timetablePeriodKey(entry: TimetableEntry): string {
		return (
			entry.period_id ||
			`${entry.start_time ?? ''}-${entry.end_time ?? ''}-${entry.period_name ?? ''}`
		);
	}

	function timetablePeriodLabel(entry: TimetableEntry): string {
		return entry.period_name || entry.title || 'ไม่ระบุคาบ';
	}

	function timetableTimeLabel(entry: TimetableEntry): string {
		if (!entry.start_time && !entry.end_time) return '';
		return `${entry.start_time?.slice(0, 5) ?? ''}-${entry.end_time?.slice(0, 5) ?? ''}`;
	}

	function timetablePeriodSort(entry: TimetableEntry): number {
		if (typeof entry.period_order_index === 'number') return entry.period_order_index;
		if (entry.start_time) {
			const [hour = '0', minute = '0'] = entry.start_time.split(':');
			return Number(hour) * 60 + Number(minute);
		}
		return 9999;
	}

	function timetableEntriesForSelectedCycle(): TimetableEntry[] {
		if (!selectedCycleDetail?.academicSemesterId) return timetableEntries;
		return timetableEntries.filter(
			(entry) => entry.academic_semester_id === selectedCycleDetail.academicSemesterId
		);
	}

	function timetablePeriodRows(): TimetablePeriodRow[] {
		const rows: TimetablePeriodRow[] = [];
		for (const entry of timetableEntriesForSelectedCycle()) {
			const key = timetablePeriodKey(entry);
			if (!rows.some((row) => row.key === key)) {
				rows.push({
					key,
					label: timetablePeriodLabel(entry),
					timeLabel: timetableTimeLabel(entry),
					sort: timetablePeriodSort(entry)
				});
			}
		}
		return rows.sort(
			(left, right) => left.sort - right.sort || left.label.localeCompare(right.label)
		);
	}

	function timetableEntryFor(day: string, row: TimetablePeriodRow): TimetableEntry | null {
		return (
			timetableEntriesForSelectedCycle().find(
				(entry) => entry.day_of_week === day && timetablePeriodKey(entry) === row.key
			) ?? null
		);
	}

	function selectTimetableEntry(entry: TimetableEntry, bookingDate: string) {
		if (!canRequest) return;
		const existingObservation = observationForTimetableCell(entry, bookingDate);
		if (existingObservation && existingObservation.status !== 'returned') return;
		selectedTimetableEntryId = entry.id;
		selectedBookingDate = bookingDate;
	}

	async function refreshTimetableForCycle(cycleId: string) {
		if (!cycleId || !canRequest || loadedTimetableCycleId === cycleId) return;
		loadingTimetable = true;
		try {
			const cycle = cycles.find((item) => item.id === cycleId);
			const timetable = await getMyTimetable({
				academic_semester_id: cycle?.academicSemesterId ?? undefined,
				include_team_ghosts: true
			});
			timetableEntries = timetable.data;
			loadedTimetableCycleId = cycleId;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถโหลดตารางสอนได้');
		} finally {
			loadingTimetable = false;
		}
	}

	function observationLessonTitle(observation: SupervisionObservation): string {
		return (
			observation.lessonSnapshot.subjectName ?? observation.manualLesson?.subjectName ?? 'คาบนิเทศ'
		);
	}

	function observationSubjectLabel(observation: SupervisionObservation): string {
		return (
			observation.lessonSnapshot.subjectName ??
			observation.manualLesson?.subjectName ??
			'ไม่ระบุวิชา'
		);
	}

	function observationClassroomLabel(observation: SupervisionObservation): string {
		return (
			observation.lessonSnapshot.classroomLabel ??
			observation.manualLesson?.classroomLabel ??
			'ไม่ระบุชั้นเรียน'
		);
	}

	function observationPeriodLabel(observation: SupervisionObservation): string {
		return (
			observation.lessonSnapshot.periodLabel ??
			observation.manualLesson?.periodLabel ??
			'ไม่ระบุคาบ'
		);
	}

	function observationRoomLabel(observation: SupervisionObservation): string {
		return observation.lessonSnapshot.roomLabel ?? observation.manualLesson?.roomLabel ?? '-';
	}

	function observationEvaluatorNames(observation: SupervisionObservation): string {
		if (observation.evaluators.length === 0) return 'ยังไม่มอบหมาย';
		return observation.evaluators
			.map((evaluator) => evaluator.evaluatorDisplayName ?? 'ผู้ประเมิน')
			.join(', ');
	}

	function observationDetailGrid(observation: SupervisionObservation): ObservationDetailItem[] {
		return [
			{ label: 'วันที่/เวลา', value: formatDate(observation.observedAt) },
			{ label: 'วิชา', value: observationSubjectLabel(observation) },
			{ label: 'คาบ', value: observationPeriodLabel(observation) },
			{ label: 'ห้อง/ชั้นเรียน', value: observationClassroomLabel(observation) },
			{ label: 'ห้องเรียน', value: observationRoomLabel(observation) },
			{ label: 'รอบนิเทศ', value: requestCycleLabel(observation) },
			{ label: 'แบบประเมิน', value: requestTemplateTitle(observation) },
			{ label: 'ผู้นิเทศ', value: observationEvaluatorNames(observation) },
			{ label: 'ส่งคำขอ', value: formatDate(observation.requestedAt) },
			{ label: 'อนุมัติ', value: formatDate(observation.approvedAt) }
		];
	}

	function combineLocalDateTime(date: string, time: string): string {
		return new Date(`${date}T${time || '00:00'}`).toISOString();
	}

	function optionalLocalDateTime(date: string, time: string): string | null {
		return date ? combineLocalDateTime(date, time) : null;
	}

	async function refreshAll() {
		loading = true;
		try {
			const [cycleItems, templateItems, structure] = await Promise.all([
				listSupervisionCycles(),
				listSupervisionTemplates(),
				getAcademicStructure()
			]);
			const observationItems = canReadObservations ? await listSupervisionObservations() : [];
			const staffItems = canManageRequests
				? await lookupStaff({ activeOnly: true, limit: 1000 })
				: [];
			cycles = cycleItems;
			templates = templateItems;
			observations = observationItems;
			staffList = staffItems;
			academicStructure = structure.data;
			selectedCycleId ||=
				cycleItems.find((cycle) => cycle.status === 'open')?.id ?? cycleItems[0]?.id ?? '';
			loadedTimetableCycleId = '';
			progressCycleId ||= cycles[0]?.id ?? '';
			cycleForm.templateId ||= templates[0]?.id ?? '';
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถโหลดข้อมูลนิเทศได้');
		} finally {
			loading = false;
		}
	}

	function replaceCycle(cycle: SupervisionCycle) {
		cycles = cycles.some((item) => item.id === cycle.id)
			? cycles.map((item) => (item.id === cycle.id ? cycle : item))
			: [cycle, ...cycles];
		selectedCycleId ||= cycle.id;
		progressCycleId ||= cycle.id;
	}

	async function refreshTemplates() {
		templates = await listSupervisionTemplates();
		cycleForm.templateId ||= templates[0]?.id ?? '';
	}

	function replaceTemplate(template: SupervisionTemplate) {
		templates = templates.some((item) => item.id === template.id)
			? templates.map((item) => (item.id === template.id ? template : item))
			: [template, ...templates];
		cycleForm.templateId ||= template.id;
	}

	function replaceObservation(observation: SupervisionObservation) {
		observations = observations.some((item) => item.id === observation.id)
			? observations.map((item) => (item.id === observation.id ? observation : item))
			: [observation, ...observations];
	}

	function currentUserEvaluator(observation: SupervisionObservation) {
		return observation.evaluators.find((evaluator) => evaluator.evaluatorUserId === currentUserId);
	}

	function requireMutationData<T>(
		response: { success: boolean; data?: T; error?: string },
		fallbackError: string
	): T {
		if (!response.success || response.data === undefined) {
			throw new Error(response.error || fallbackError);
		}

		return response.data;
	}

	async function createBookingRequest() {
		if (!canRequest) return;
		if (!currentBookingCycle) {
			toast.error('ยังไม่มีรอบนิเทศที่เปิดให้จองในขณะนี้');
			return;
		}

		if (!manualMode && (!selectedTimetableEntryId || !selectedBookingDate)) {
			toast.error('เลือกคาบจากตารางสอนก่อน');
			return;
		}

		if (
			manualMode &&
			(!manualLesson.subjectName ||
				!manualLesson.classroomLabel ||
				!manualLessonDate ||
				!manualLessonTime)
		) {
			toast.error('กรอกข้อมูลคาบแบบกำหนดเองให้ครบ');
			return;
		}

		savingAction = 'request-booking';
		try {
			const response = await requestSupervisionObservation({
				cycleId: currentBookingCycle.id,
				timetableEntryId: manualMode ? null : selectedTimetableEntryId,
				observedAt:
					manualMode || !selectedTimetableEntry
						? null
						: timetableObservedAt(selectedTimetableEntry, selectedBookingDate),
				manualLesson: manualMode
					? {
							subjectName: manualLesson.subjectName,
							classroomLabel: manualLesson.classroomLabel,
							roomLabel: manualLesson.roomLabel || null,
							observedAt: combineLocalDateTime(manualLessonDate, manualLessonTime),
							periodLabel: manualLesson.periodLabel,
							reason: manualLesson.reason
						}
					: null
			});
			const observation = requireMutationData(response, 'ส่งคำขอไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('ส่งคำขอจองนิเทศแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ส่งคำขอไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	function requestCycleLabel(observation: SupervisionObservation): string {
		const cycle = cycles.find((item) => item.id === observation.cycleId);
		return cycle ? cycleLabel(cycle) : 'ไม่พบรอบนิเทศ';
	}

	function requestTemplateTitle(observation: SupervisionObservation): string {
		return (
			templates.find((template) => template.id === observation.templateId)?.title ??
			'ไม่พบแบบประเมิน'
		);
	}

	function selectedRequestEvaluatorIds(observationId: string): string[] {
		return requestEvaluatorIds[observationId] ?? [];
	}

	function selectedRequestEvaluators(observationId: string): StaffLookupItem[] {
		const selectedIds = new Set(selectedRequestEvaluatorIds(observationId));
		return staffList.filter((staff) => selectedIds.has(staff.id));
	}

	function requestEvaluatorPickerOpen(observationId: string): boolean {
		return Boolean(evaluatorPickerOpenByRequest[observationId]);
	}

	function setRequestEvaluatorPickerOpen(observationId: string, open: boolean) {
		evaluatorPickerOpenByRequest = {
			...evaluatorPickerOpenByRequest,
			[observationId]: open
		};
	}

	function toggleRequestEvaluatorForRequest(observationId: string, staff: StaffLookupItem) {
		if (!canManageRequests) return;
		const currentIds = selectedRequestEvaluatorIds(observationId);
		requestEvaluatorIds = {
			...requestEvaluatorIds,
			[observationId]: currentIds.includes(staff.id)
				? currentIds.filter((id) => id !== staff.id)
				: [...currentIds, staff.id]
		};
	}

	function removeRequestEvaluatorForRequest(observationId: string, evaluatorId: string) {
		requestEvaluatorIds = {
			...requestEvaluatorIds,
			[observationId]: selectedRequestEvaluatorIds(observationId).filter((id) => id !== evaluatorId)
		};
	}

	function setRequestReturnCommentForRequest(observationId: string, comment: string) {
		requestReturnComments = {
			...requestReturnComments,
			[observationId]: comment
		};
	}

	function clearRequestApprovalState(observationId: string) {
		const { [observationId]: _evaluatorIds, ...remainingEvaluatorIds } = requestEvaluatorIds;
		const { [observationId]: _comment, ...remainingComments } = requestReturnComments;
		const { [observationId]: _pickerOpen, ...remainingPickerOpen } = evaluatorPickerOpenByRequest;
		requestEvaluatorIds = remainingEvaluatorIds;
		requestReturnComments = remainingComments;
		evaluatorPickerOpenByRequest = remainingPickerOpen;
	}

	async function approveRequest(observationId: string) {
		if (!canManageRequests) return;
		const evaluatorIds = selectedRequestEvaluatorIds(observationId);
		if (evaluatorIds.length === 0) {
			toast.error('เลือกผู้ประเมินอย่างน้อย 1 คนก่อนอนุมัติ');
			return;
		}

		savingAction = `approve-request:${observationId}`;
		try {
			const response = await approveSupervisionObservationRequest(observationId, {
				evaluators: evaluatorIds.map((evaluatorId) => ({
					evaluatorUserId: evaluatorId,
					isRequired: true
				}))
			});
			const observation = requireMutationData(response, 'อนุมัติคำขอไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('อนุมัติคำขอและมอบหมายผู้ประเมินแล้ว');
			clearRequestApprovalState(observationId);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'อนุมัติคำขอไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function returnRequest(id: string) {
		if (!canManageRequests) return;
		savingAction = `return-request:${id}`;
		try {
			const response = await returnSupervisionObservationRequest(id, {
				comment: requestReturnComments[id] || null
			});
			const observation = requireMutationData(response, 'ส่งกลับคำขอไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('ส่งกลับคำขอแล้ว');
			clearRequestApprovalState(id);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ส่งกลับคำขอไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	function prepareEvaluationDraft(observation: SupervisionObservation) {
		if (!canEvaluate) return;
		evaluationObservationId = observation.id;
		const template = templates.find((item) => item.id === observation.templateId);
		const nextDrafts: { [itemId: string]: ResponseDraft } = {};
		for (const section of template?.sections ?? []) {
			for (const item of section.items) {
				nextDrafts[item.id] = { ratingScore: '', textResponse: '' };
			}
		}
		responseDrafts = nextDrafts;
		evaluationDialogOpen = true;
	}

	function clearEvaluationDraft() {
		evaluationDialogOpen = false;
		evaluationObservationId = '';
		responseDrafts = {};
	}

	function setEvaluationDialogOpen(open: boolean) {
		evaluationDialogOpen = open;
		if (!open) {
			clearEvaluationDraft();
		}
	}

	function ratingScale(min: number, max: number): number[] {
		const start = Math.trunc(Number(min));
		const end = Math.trunc(Number(max));
		if (!Number.isFinite(start) || !Number.isFinite(end) || start > end) return [];
		return Array.from({ length: end - start + 1 }, (_, index) => start + index);
	}

	function updateDraft(itemId: string, patch: Partial<ResponseDraft>) {
		if (!canEvaluate) return;
		responseDrafts = {
			...responseDrafts,
			[itemId]: {
				...(responseDrafts[itemId] ?? { ratingScore: '', textResponse: '' }),
				...patch
			}
		};
	}

	function evaluationPayload(): SaveEvaluationRequest {
		const responses = [];
		for (const section of selectedEvaluationRubricSections) {
			for (const item of section.items) {
				const draft = responseDrafts[item.localId] ?? { ratingScore: '', textResponse: '' };
				if (item.itemType === 'rating') {
					responses.push({
						templateItemId: item.localId,
						ratingScore: draft.ratingScore ? Number(draft.ratingScore) : null,
						textResponse: null
					});
				} else {
					responses.push({
						templateItemId: item.localId,
						ratingScore: null,
						textResponse: draft.textResponse || null
					});
				}
			}
		}
		return { responses };
	}

	function missingRequiredEvaluationLabels(): string[] {
		const missing: string[] = [];
		for (const section of selectedEvaluationRubricSections) {
			for (const item of section.items) {
				if (!item.required) continue;
				const draft = responseDrafts[item.localId] ?? { ratingScore: '', textResponse: '' };
				const answered =
					item.itemType === 'rating'
						? Boolean(draft.ratingScore)
						: Boolean(draft.textResponse.trim());
				if (!answered) missing.push(item.label);
			}
		}
		return missing;
	}

	async function saveEvaluation(submit = false) {
		if (!canEvaluate) return;
		if (!evaluationObservationId) {
			toast.error('เลือกรายการประเมินก่อน');
			return;
		}
		if (submit) {
			const missing = missingRequiredEvaluationLabels();
			if (missing.length > 0) {
				toast.error(`กรอกหัวข้อบังคับให้ครบ (${missing.length} ข้อ)`);
				return;
			}
		}

		savingEvaluation = submit ? 'submit' : 'draft';
		try {
			const payload = evaluationPayload();
			const response = submit
				? await submitMySupervisionEvaluation(evaluationObservationId, payload)
				: await saveMySupervisionEvaluation(evaluationObservationId, payload);
			const observation = requireMutationData(response, 'บันทึกผลประเมินไม่สำเร็จ');
			replaceObservation(observation);
			clearEvaluationDraft();
			toast.success(submit ? 'ส่งผลประเมินแล้ว' : 'บันทึกแบบร่างแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกผลประเมินไม่สำเร็จ');
		} finally {
			savingEvaluation = null;
		}
	}

	async function submitForReview(id: string) {
		if (!canManageRequests) return;
		savingAction = `submit-review:${id}`;
		try {
			const response = await submitSupervisionObservationForReview(id);
			const observation = requireMutationData(response, 'ส่งตรวจทานไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('ส่งตรวจทานแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ส่งตรวจทานไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function approveResult(id: string) {
		if (!canApprove) return;
		savingAction = `approve-result:${id}`;
		try {
			const response = await approveSupervisionObservation(id);
			const observation = requireMutationData(response, 'อนุมัติผลไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('อนุมัติผลนิเทศแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'อนุมัติผลไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function publishResult(id: string) {
		if (!canApprove) return;
		savingAction = `publish-result:${id}`;
		try {
			const response = await publishSupervisionObservation(id);
			const observation = requireMutationData(response, 'เผยแพร่ผลไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('เผยแพร่ผลนิเทศแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'เผยแพร่ผลไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function returnResult(id: string) {
		if (!canApprove) return;
		savingAction = `return-result:${id}`;
		try {
			const response = await returnSupervisionObservation(id, { comment: reviewComment || null });
			const observation = requireMutationData(response, 'ส่งกลับผลไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('ส่งกลับผลนิเทศแล้ว');
			reviewComment = '';
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ส่งกลับผลไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function acknowledgeResult(id: string) {
		savingAction = `acknowledge-result:${id}`;
		try {
			const response = await acknowledgeSupervisionObservation(id, {
				comment: acknowledgeComment || null
			});
			const observation = requireMutationData(response, 'รับทราบผลไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('รับทราบผลนิเทศแล้ว');
			acknowledgeComment = '';
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'รับทราบผลไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function createCycle() {
		if (!canManageSchool) return;
		if (
			!cycleForm.title ||
			!cycleForm.templateId ||
			!cycleForm.academicSemesterId ||
			!cycleForm.startsDate ||
			!cycleForm.endsDate
		) {
			toast.error('กรอกชื่อรอบ ภาคเรียน แบบประเมิน และช่วงวันที่ให้ครบ');
			return;
		}

		const payload: CreateSupervisionCycleRequest = {
			academicYear: Number(cycleForm.academicYear),
			semester: cycleForm.semester,
			academicSemesterId: cycleForm.academicSemesterId,
			title: cycleForm.title,
			description: cycleForm.description || null,
			templateId: cycleForm.templateId,
			bookingOpensAt: optionalLocalDateTime(cycleForm.bookingOpensDate, cycleForm.bookingOpensTime),
			bookingClosesAt: optionalLocalDateTime(
				cycleForm.bookingClosesDate,
				cycleForm.bookingClosesTime
			),
			startsAt: combineLocalDateTime(cycleForm.startsDate, cycleForm.startsTime),
			endsAt: combineLocalDateTime(cycleForm.endsDate, cycleForm.endsTime),
			status: cycleForm.status,
			targets: [{ targetType: 'school', requiredObservations: 1, priority: 100 }]
		};

		savingAction = 'create-cycle';
		try {
			const response = await createSupervisionCycle(payload);
			const cycle = requireMutationData(response, 'สร้างรอบนิเทศไม่สำเร็จ');
			replaceCycle(cycle);
			toast.success('สร้างรอบนิเทศแล้ว');
			cycleForm.title = '';
			cycleForm.description = '';
			createCycleDialogOpen = false;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'สร้างรอบนิเทศไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function setCycleStatus(cycle: SupervisionCycle, status: SupervisionCycleStatus) {
		if (!canManageSchool) return;
		if (cycle.status === status) return;

		savingAction = `cycle-status:${cycle.id}:${status}`;
		try {
			const response = await updateSupervisionCycle(cycle.id, { status });
			const updatedCycle = requireMutationData(response, 'เปลี่ยนสถานะรอบนิเทศไม่สำเร็จ');
			replaceCycle(updatedCycle);
			toast.success(`เปลี่ยนสถานะรอบนิเทศเป็น${statusLabel(status)}แล้ว`);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'เปลี่ยนสถานะรอบนิเทศไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	function normalizeTemplateSections(sections: RubricFormSection[]): RubricFormSection[] {
		return sections.map((section, sectionIndex) => ({
			...section,
			sortOrder: sectionIndex + 1,
			items: section.items.map((item, itemIndex) => ({
				...item,
				sortOrder: itemIndex + 1
			}))
		}));
	}

	function resetTemplateForm() {
		editingTemplateId = '';
		templateForm = createDefaultTemplateForm();
	}

	function openCreateTemplateDialog() {
		resetTemplateForm();
		createTemplateDialogOpen = true;
	}

	function openEditTemplateDialog(template: SupervisionTemplate) {
		editingTemplateId = template.id;
		templateForm = {
			title: template.title,
			description: template.description ?? '',
			status: template.status,
			ratingMin: template.ratingMin,
			ratingMax: template.ratingMax,
			sections: templateSectionsToRubricForm(template)
		};
		createTemplateDialogOpen = true;
	}

	function loadPaperTemplatePreset() {
		templateForm = {
			...templateForm,
			title: templateForm.title || 'แบบนิเทศการจัดการเรียนรู้',
			description:
				templateForm.description || 'แบบประเมินการจัดการเรียนรู้ตามหัวข้อการนิเทศในชั้นเรียน',
			ratingMin: 1,
			ratingMax: 5,
			sections: createPaperSupervisionRubricSections()
		};
	}

	function updateTemplateSection(sectionLocalId: string, patch: Partial<RubricFormSection>) {
		templateForm = {
			...templateForm,
			sections: templateForm.sections.map((section) =>
				section.localId === sectionLocalId ? { ...section, ...patch } : section
			)
		};
	}

	function updateTemplateItem(
		sectionLocalId: string,
		itemLocalId: string,
		patch: Partial<RubricFormSection['items'][number]>
	) {
		templateForm = {
			...templateForm,
			sections: templateForm.sections.map((section) =>
				section.localId === sectionLocalId
					? {
							...section,
							items: section.items.map((item) =>
								item.localId === itemLocalId ? { ...item, ...patch } : item
							)
						}
					: section
			)
		};
	}

	function moveItemInList<T>(items: T[], index: number, direction: -1 | 1): T[] {
		const targetIndex = index + direction;
		if (index < 0 || targetIndex < 0 || targetIndex >= items.length) return items;
		const next = [...items];
		const [item] = next.splice(index, 1);
		next.splice(targetIndex, 0, item);
		return next;
	}

	function addTemplateSection() {
		templateForm = {
			...templateForm,
			sections: normalizeTemplateSections([
				...templateForm.sections,
				createBlankRubricSection(templateForm.sections.length + 1)
			])
		};
	}

	function removeTemplateSection(sectionLocalId: string) {
		if (templateForm.sections.length <= 1) {
			toast.error('แบบประเมินต้องมีอย่างน้อย 1 หมวด');
			return;
		}
		templateForm = {
			...templateForm,
			sections: normalizeTemplateSections(
				templateForm.sections.filter((section) => section.localId !== sectionLocalId)
			)
		};
	}

	function moveTemplateSection(sectionLocalId: string, direction: -1 | 1) {
		const index = templateForm.sections.findIndex((section) => section.localId === sectionLocalId);
		templateForm = {
			...templateForm,
			sections: normalizeTemplateSections(moveItemInList(templateForm.sections, index, direction))
		};
	}

	function addTemplateItem(sectionLocalId: string, itemType: RubricItemType) {
		templateForm = {
			...templateForm,
			sections: templateForm.sections.map((section) =>
				section.localId === sectionLocalId
					? {
							...section,
							items: section.items
								.map((item, index) => ({ ...item, sortOrder: index + 1 }))
								.concat(createBlankRubricItem(itemType, section.items.length + 1))
						}
					: section
			)
		};
	}

	function removeTemplateItem(sectionLocalId: string, itemLocalId: string) {
		templateForm = {
			...templateForm,
			sections: templateForm.sections.map((section) =>
				section.localId === sectionLocalId
					? {
							...section,
							items: section.items
								.filter((item) => item.localId !== itemLocalId)
								.map((item, index) => ({ ...item, sortOrder: index + 1 }))
						}
					: section
			)
		};
	}

	function moveTemplateItem(sectionLocalId: string, itemLocalId: string, direction: -1 | 1) {
		templateForm = {
			...templateForm,
			sections: templateForm.sections.map((section) => {
				if (section.localId !== sectionLocalId) return section;
				const index = section.items.findIndex((item) => item.localId === itemLocalId);
				return {
					...section,
					items: moveItemInList(section.items, index, direction).map((item, itemIndex) => ({
						...item,
						sortOrder: itemIndex + 1
					}))
				};
			})
		};
	}

	function templatePayload(): CreateSupervisionTemplateRequest {
		return {
			title: templateForm.title.trim(),
			description: templateForm.description.trim() || null,
			status: templateForm.status,
			ratingMin: Number(templateForm.ratingMin),
			ratingMax: Number(templateForm.ratingMax),
			sections: normalizeTemplateSections(templateForm.sections).map((section) => ({
				title: section.title.trim(),
				description: section.description.trim() || null,
				sortOrder: section.sortOrder,
				items: section.items.map((item) => ({
					label: item.label.trim(),
					description: item.description.trim() || null,
					itemType: item.itemType,
					required: item.required,
					sortOrder: item.sortOrder
				}))
			})),
			steps: [
				{
					stepOrder: 1,
					stepCode: 'approve',
					label: 'อนุมัติผลนิเทศ',
					actorKind: 'permission',
					actorPermission: PERMISSIONS.SUPERVISION_APPROVE_SCHOOL,
					actionKind: 'approve',
					required: true
				},
				{
					stepOrder: 2,
					stepCode: 'acknowledge',
					label: 'ครูรับทราบผล',
					actorKind: 'observed_teacher',
					actionKind: 'acknowledge',
					required: true
				}
			]
		};
	}

	function validateTemplateForm(): boolean {
		if (!templateForm.title.trim()) {
			toast.error('กรอกชื่อแบบประเมิน');
			return false;
		}
		if (Number(templateForm.ratingMin) >= Number(templateForm.ratingMax)) {
			toast.error('คะแนนต่ำสุดต้องน้อยกว่าคะแนนสูงสุด');
			return false;
		}
		if (templateForm.sections.length === 0) {
			toast.error('เพิ่มหมวดแบบประเมินอย่างน้อย 1 หมวด');
			return false;
		}
		if (templateForm.sections.every((section) => section.items.length === 0)) {
			toast.error('เพิ่มหัวข้อประเมินอย่างน้อย 1 ข้อ');
			return false;
		}
		if (templateForm.sections.some((section) => !section.title.trim())) {
			toast.error('กรอกชื่อหมวดให้ครบ');
			return false;
		}
		if (templateForm.sections.some((section) => section.items.some((item) => !item.label.trim()))) {
			toast.error('กรอกหัวข้อประเมินให้ครบ');
			return false;
		}
		return true;
	}

	async function createTemplate() {
		if (!canManageSchool) return;
		if (!validateTemplateForm()) return;

		savingTemplate = true;
		try {
			const payload = templatePayload();
			const response = editingTemplateId
				? await updateSupervisionTemplate(editingTemplateId, payload)
				: await createSupervisionTemplate(payload);
			if (!response.success) throw new Error(response.error || 'บันทึกแบบประเมินไม่สำเร็จ');
			if (response.data) {
				replaceTemplate(response.data);
			} else {
				await refreshTemplates();
			}
			toast.success(editingTemplateId ? 'แก้ไขแบบประเมินนิเทศแล้ว' : 'สร้างแบบประเมินนิเทศแล้ว');
			resetTemplateForm();
			createTemplateDialogOpen = false;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกแบบประเมินไม่สำเร็จ');
		} finally {
			savingTemplate = false;
		}
	}

	async function loadProgress() {
		if (!canReport) return;
		if (!progressCycleId) {
			toast.error('เลือกรอบนิเทศก่อน');
			return;
		}

		saving = true;
		try {
			progress = await getSupervisionCycleProgress(progressCycleId);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'โหลดรายงานไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	onMount(() => {
		void refreshAll();
	});
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<PageShell
	title="นิเทศการสอน"
	description="จัดรอบนิเทศ จองคาบ ประเมิน ส่งตรวจทาน และรับทราบผลในพื้นที่เดียว"
>
	{#snippet actions()}
		<Button variant="outline" size="sm" onclick={refreshAll} disabled={loading || mutationBusy}>
			<RefreshCw class={cn('mr-2 h-4 w-4', loading && 'animate-spin')} />
			รีเฟรช
		</Button>
		{#if canManageSchool}
			<Button size="sm" onclick={() => (createCycleDialogOpen = true)}>
				<Plus class="mr-2 h-4 w-4" />
				สร้างรอบนิเทศ
			</Button>
			<Button size="sm" variant="outline" onclick={openCreateTemplateDialog}>
				<Settings2 class="mr-2 h-4 w-4" />
				สร้างแบบประเมิน
			</Button>
		{/if}
	{/snippet}

	<div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
		<div class="rounded-md border bg-background px-3 py-2 text-center">
			<p class="text-lg font-semibold">{cycles.length}</p>
			<p class="text-xs text-muted-foreground">รอบนิเทศ</p>
		</div>
		<div class="rounded-md border bg-background px-3 py-2 text-center">
			<p class="text-lg font-semibold">{requestedObservations.length}</p>
			<p class="text-xs text-muted-foreground">คำขอรออนุมัติ</p>
		</div>
		<div class="rounded-md border bg-background px-3 py-2 text-center">
			<p class="text-lg font-semibold">{activeAssignedObservations.length}</p>
			<p class="text-xs text-muted-foreground">รอประเมิน</p>
		</div>
		<div class="rounded-md border bg-background px-3 py-2 text-center">
			<p class="text-lg font-semibold">{templates.length}</p>
			<p class="text-xs text-muted-foreground">แบบประเมิน</p>
		</div>
	</div>

	{#if loading}
		<PageSkeleton variant="detail" />
	{/if}

	<Tabs.Root bind:value={activeTab} class="space-y-4">
		<Tabs.List class="grid w-full grid-cols-2 md:grid-cols-6">
			<Tabs.Trigger value="mine">ของฉัน</Tabs.Trigger>
			<Tabs.Trigger value="requests" disabled={!canManageRequests}>คำขอจอง</Tabs.Trigger>
			<Tabs.Trigger value="evaluate" disabled={!canEvaluate}>ประเมิน</Tabs.Trigger>
			<Tabs.Trigger value="cycles" disabled={!canManageSchool}>รอบนิเทศ</Tabs.Trigger>
			<Tabs.Trigger value="templates" disabled={!canManageSchool}>แบบประเมิน</Tabs.Trigger>
			<Tabs.Trigger value="reports" disabled={!canReport}>รายงาน</Tabs.Trigger>
		</Tabs.List>

		<Tabs.Content value="mine" class="space-y-4">
			<Card.Root>
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<BookOpenCheck class="h-5 w-5" />
						จองคาบนิเทศของฉัน
					</Card.Title>
					<Card.Description>เลือกคาบสอนจริงจากตาราง หรือใช้คาบกำหนดเองเมื่อจำเป็น</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-4">
					{#if !canRequest}
						<PageState
							variant="permission"
							title="ยังไม่มีสิทธิ์จองคาบนิเทศ"
							description="ต้องมีสิทธิ์จองคาบนิเทศของตนเองก่อนจึงจะส่งคำขอได้"
						/>
					{:else}
						<div class="grid gap-4 lg:grid-cols-[1fr_auto]">
							<div class="rounded-md border bg-muted/20 p-3">
								<Label>รอบนิเทศปัจจุบัน</Label>
								{#if currentBookingCycle}
									<div class="mt-1 flex flex-wrap items-center gap-2">
										<p class="font-medium">{cycleLabel(currentBookingCycle)}</p>
										<Badge variant="secondary">{statusLabel(currentBookingCycle.status)}</Badge>
									</div>
									<p class="mt-1 text-xs text-muted-foreground">
										จองได้ {formatDate(currentBookingCycle.bookingOpensAt)} - {formatDate(
											currentBookingCycle.bookingClosesAt
										)}
									</p>
								{:else}
									<p class="mt-1 text-sm text-muted-foreground">
										ยังไม่มีรอบนิเทศที่เปิดให้จองในขณะนี้
									</p>
								{/if}
							</div>
							<div class="space-y-2">
								<Label>รูปแบบคาบ</Label>
								<div class="flex gap-2">
									<Button
										type="button"
										variant={manualMode ? 'outline' : 'default'}
										size="sm"
										onclick={() => (manualMode = false)}
									>
										จากตารางสอน
									</Button>
									<Button
										type="button"
										variant={manualMode ? 'default' : 'outline'}
										size="sm"
										onclick={() => (manualMode = true)}
									>
										กำหนดเอง
									</Button>
								</div>
							</div>
						</div>

						{#if !manualMode}
							<div class="space-y-2">
								<div class="flex flex-col gap-2 lg:flex-row lg:items-end lg:justify-between">
									<div>
										<Label>คาบจากตารางสอน</Label>
										{#if currentBookingCycle?.academicSemesterId}
											<p class="text-xs text-muted-foreground">
												แสดงคาบสอนจาก {semesterLabel(currentBookingCycle.academicSemesterId)}
											</p>
										{/if}
									</div>
									<div class="flex flex-wrap items-center gap-2">
										<Button
											type="button"
											variant="outline"
											size="sm"
											onclick={goToPreviousBookingWeek}
											disabled={!canNavigateBookingWeek(-1)}
										>
											<ArrowUp class="mr-1 h-4 w-4" />
											สัปดาห์ก่อน
										</Button>
										<Badge variant="outline" class="px-3 py-1">{bookingWeekLabel()}</Badge>
										<Button
											type="button"
											variant="outline"
											size="sm"
											onclick={resetToCurrentBookingWeek}
										>
											สัปดาห์นี้
										</Button>
										<Button
											type="button"
											variant="outline"
											size="sm"
											onclick={goToNextBookingWeek}
											disabled={!canNavigateBookingWeek(1)}
										>
											สัปดาห์ถัดไป
											<ArrowDown class="ml-1 h-4 w-4" />
										</Button>
									</div>
								</div>
								{#if loadingTimetable}
									<Alert.Root>
										<Loader2 class="h-4 w-4 animate-spin" />
										<Alert.Title>กำลังโหลดตารางสอน</Alert.Title>
										<Alert.Description>ระบบกำลังโหลดคาบสอนตามภาคเรียนของรอบนิเทศ</Alert.Description>
									</Alert.Root>
								{:else if !currentBookingCycle}
									<Alert.Root>
										<Alert.Title>ยังไม่มีรอบที่เปิดให้จอง</Alert.Title>
										<Alert.Description>
											เมื่อฝ่ายวิชาการเปิดรอบนิเทศในช่วงเวลาปัจจุบัน ตารางจองจะแสดงอัตโนมัติ
										</Alert.Description>
									</Alert.Root>
								{:else if timetableEntriesForSelectedCycle().length === 0}
									<Alert.Root>
										<Alert.Title>ไม่พบคาบสอนในภาคเรียนนี้</Alert.Title>
										<Alert.Description>
											ตรวจสอบตารางสอนของครู หรือใช้คาบกำหนดเองเมื่อจำเป็น
										</Alert.Description>
									</Alert.Root>
								{:else}
									<div class="overflow-x-auto rounded-md border">
										<Table.Root>
											<Table.Header>
												<Table.Row>
													<Table.Head class="sticky left-0 z-10 w-[112px] bg-background"
														>วัน</Table.Head
													>
													{#each timetablePeriodRows() as row (row.key)}
														<Table.Head class="min-w-[150px] text-center">
															<div class="font-medium">{row.label}</div>
															{#if row.timeLabel}
																<div class="text-xs font-normal text-muted-foreground">
																	{row.timeLabel}
																</div>
															{/if}
														</Table.Head>
													{/each}
												</Table.Row>
											</Table.Header>
											<Table.Body>
												{#each bookingWeekDays as day (day.value)}
													<Table.Row>
														<Table.Cell class="sticky left-0 z-10 bg-background align-top">
															<div class="font-medium">{day.label}</div>
															<div class="text-xs text-muted-foreground">
																{formatShortDate(day.date)}
															</div>
														</Table.Cell>
														{#each timetablePeriodRows() as row (row.key)}
															{@const entry = timetableEntryFor(day.value, row)}
															{@const cellObservation = entry
																? observationForTimetableCell(entry, day.date)
																: null}
															{@const isOutsideCycle = !bookingDateInCycle(day.date)}
															<Table.Cell class="min-w-[150px] p-1 align-top">
																{#if entry}
																	<button
																		type="button"
																		class={cn(
																			'min-h-20 w-full rounded-md border p-2 text-left transition hover:border-primary hover:bg-primary/5',
																			selectedTimetableEntryId === entry.id &&
																				selectedBookingDate === day.date &&
																				'border-primary bg-primary/10 shadow-sm'
																		)}
																		disabled={isOutsideCycle ||
																			(!!cellObservation && cellObservation.status !== 'returned')}
																		onclick={() => selectTimetableEntry(entry, day.date)}
																	>
																		<div class="flex items-start justify-between gap-2">
																			<div class="text-sm font-medium leading-snug">
																				{timetableEntryTitle(entry)}
																			</div>
																			{#if cellObservation}
																				<Badge variant="secondary" class="shrink-0 text-[10px]">
																					{statusLabel(cellObservation.status)}
																				</Badge>
																			{:else if isOutsideCycle}
																				<Badge variant="outline" class="shrink-0 text-[10px]">
																					นอกช่วง
																				</Badge>
																			{/if}
																		</div>
																		<p class="mt-1 text-xs text-muted-foreground">
																			{entry.period_name ?? row.label}
																		</p>
																		<p class="mt-1 text-xs text-muted-foreground">
																			{entry.classroom_name ?? '-'}
																		</p>
																		{#if entry.room_code}
																			<p class="text-xs text-muted-foreground">
																				ห้อง {entry.room_code}
																			</p>
																		{/if}
																	</button>
																{:else}
																	<div
																		class="min-h-20 rounded-md border border-dashed bg-muted/20"
																	></div>
																{/if}
															</Table.Cell>
														{/each}
													</Table.Row>
												{/each}
											</Table.Body>
										</Table.Root>
									</div>
								{/if}
								{#if selectedTimetableEntry}
									<p class="text-xs text-muted-foreground">
										เลือกแล้ว: {formatShortDate(selectedBookingDate)} · {timetableLabel(
											selectedTimetableEntry
										)}
									</p>
								{/if}
							</div>
						{:else}
							<div class="grid gap-3 lg:grid-cols-2">
								<div class="space-y-2">
									<Label>รายวิชา</Label>
									<Input bind:value={manualLesson.subjectName} placeholder="ชื่อรายวิชา" />
								</div>
								<div class="space-y-2">
									<Label>ห้องเรียน</Label>
									<Input bind:value={manualLesson.classroomLabel} placeholder="เช่น ม.3/1" />
								</div>
								<div class="space-y-2">
									<Label>วันที่นิเทศ</Label>
									<DatePicker bind:value={manualLessonDate} placeholder="เลือกวันที่" />
								</div>
								<div class="space-y-2">
									<Label>เวลา</Label>
									<Input type="time" bind:value={manualLessonTime} />
								</div>
								<div class="space-y-2">
									<Label>คาบ/ห้อง</Label>
									<div class="grid grid-cols-2 gap-2">
										<Input bind:value={manualLesson.periodLabel} placeholder="คาบที่ 2" />
										<Input bind:value={manualLesson.roomLabel} placeholder="ห้อง 321" />
									</div>
								</div>
								<div class="space-y-2 lg:col-span-2">
									<Label>เหตุผลที่ใช้คาบกำหนดเอง</Label>
									<Textarea bind:value={manualLesson.reason} rows={3} />
								</div>
							</div>
						{/if}

						<LoadingButton
							onclick={createBookingRequest}
							loading={savingAction === 'request-booking'}
							loadingLabel="กำลังส่ง..."
							disabled={loading || mutationBusy}
						>
							<Send class="mr-2 h-4 w-4" />
							ส่งคำขอจอง
						</LoadingButton>
					{/if}
				</Card.Content>
			</Card.Root>

			<Card.Root>
				<Card.Header>
					<Card.Title>รายการของฉัน</Card.Title>
				</Card.Header>
				<Card.Content>
					{#if myObservations.length === 0}
						<PageState
							title="ยังไม่มีรายการนิเทศของฉัน"
							description="เมื่อส่งคำขอหรือได้รับผลนิเทศ รายการจะแสดงที่นี่"
						/>
					{:else}
						<div class="space-y-3" data-supervision-own-list="cards">
							{#each myObservations as observation (observation.id)}
								<div class="rounded-md border bg-background p-4">
									<div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
										<div class="min-w-0 space-y-1">
											<div class="flex flex-wrap items-center gap-2">
												<h3 class="font-semibold">{observationSubjectLabel(observation)}</h3>
												<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
											</div>
											<p class="text-sm text-muted-foreground">
												{observationPeriodLabel(observation)} · {observationClassroomLabel(
													observation
												)}
											</p>
										</div>
										<div class="shrink-0 text-sm text-muted-foreground">
											{formatDate(observation.observedAt)}
										</div>
									</div>

									<div class="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-5">
										{#each observationDetailGrid(observation) as detail (detail.label)}
											<div>
												<p class="text-xs text-muted-foreground">{detail.label}</p>
												<p class="font-medium">{detail.value}</p>
											</div>
										{/each}
									</div>

									<div
										class="mt-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between"
									>
										<div class="min-w-0">
											<p class="text-xs text-muted-foreground">ผู้นิเทศ</p>
											<div class="mt-1 flex flex-wrap gap-2">
												{#if observation.evaluators.length === 0}
													<Badge variant="outline">ยังไม่มอบหมาย</Badge>
												{:else}
													{#each observation.evaluators as evaluator (evaluator.id)}
														<Badge variant="secondary">
															{evaluator.evaluatorDisplayName ?? 'ผู้ประเมิน'}
														</Badge>
													{/each}
												{/if}
											</div>
										</div>
										<div class="flex shrink-0 flex-wrap gap-2">
											<Button
												size="sm"
												variant="outline"
												href={`/staff/academic/supervision/${observation.id}`}
											>
												<Eye class="h-4 w-4" />
												รายละเอียด
											</Button>
											{#if observation.status === 'published'}
												<Dialog.Root>
													<Dialog.Trigger>
														{#snippet child({ props })}
															<Button size="sm" {...props}>
																<FileSignature class="mr-2 h-4 w-4" />
																รับทราบผล
															</Button>
														{/snippet}
													</Dialog.Trigger>
													<Dialog.Content>
														<Dialog.Header>
															<Dialog.Title>รับทราบผลนิเทศ</Dialog.Title>
															<Dialog.Description>
																เพิ่มความคิดเห็นได้ถ้าต้องการ แล้วกดยืนยันรับทราบผลนิเทศ
															</Dialog.Description>
														</Dialog.Header>
														<Textarea
															bind:value={acknowledgeComment}
															rows={3}
															placeholder="ความคิดเห็นเพิ่มเติม (ถ้ามี)"
														/>
														<Dialog.Footer>
															<LoadingButton
																onclick={() => acknowledgeResult(observation.id)}
																loading={savingAction === `acknowledge-result:${observation.id}`}
																loadingLabel="กำลังบันทึก..."
																disabled={mutationBusy}
															>
																ยืนยันรับทราบ
															</LoadingButton>
														</Dialog.Footer>
													</Dialog.Content>
												</Dialog.Root>
											{:else}
												<span class="text-sm text-muted-foreground">-</span>
											{/if}
										</div>
									</div>
								</div>
							{/each}
						</div>
					{/if}
				</Card.Content>
			</Card.Root>
		</Tabs.Content>

		<Tabs.Content value="requests" class="space-y-4">
			<Card.Root>
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<UserCheck class="h-5 w-5" />
						คำขอจองที่รออนุมัติ
					</Card.Title>
					<Card.Description>
						ตรวจข้อมูลการจอง มอบหมายผู้ประเมินได้หลายคน แล้วอนุมัติทีละรายการ
					</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-4">
					{#if requestedObservations.length === 0}
						<PageState
							title="ไม่มีคำขอจองที่รออนุมัติ"
							description="เมื่อครูส่งคำขอจอง รายการจะปรากฏในส่วนนี้"
						/>
					{:else}
						<div class="space-y-3">
							{#each requestedObservations as observation (observation.id)}
								<div class="rounded-md border bg-background p-4">
									<div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
										<div class="min-w-0 space-y-1">
											<div class="flex flex-wrap items-center gap-2">
												<h3 class="font-semibold">
													{observation.observedDisplayName ?? 'ครูผู้ขอรับนิเทศ'}
												</h3>
												<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
											</div>
											<p class="text-sm text-muted-foreground">
												{observationLessonTitle(observation)}
											</p>
										</div>
										<div class="text-sm text-muted-foreground">
											ส่งคำขอ {formatDate(observation.requestedAt)}
										</div>
									</div>

									<div class="mt-4 grid gap-3 text-sm md:grid-cols-2 xl:grid-cols-3">
										<div>
											<p class="text-xs text-muted-foreground">วันที่นิเทศ</p>
											<p class="font-medium">{formatDate(observation.observedAt)}</p>
										</div>
										<div>
											<p class="text-xs text-muted-foreground">รอบนิเทศ</p>
											<p class="font-medium">{requestCycleLabel(observation)}</p>
										</div>
										<div>
											<p class="text-xs text-muted-foreground">แบบประเมิน</p>
											<p class="font-medium">{requestTemplateTitle(observation)}</p>
										</div>
										<div>
											<p class="text-xs text-muted-foreground">คาบ/ห้องเรียน</p>
											<p class="font-medium">{observationLessonTitle(observation)}</p>
										</div>
										<div>
											<p class="text-xs text-muted-foreground">ผู้ขอรับนิเทศ</p>
											<p class="font-medium">{observation.observedDisplayName ?? '-'}</p>
										</div>
										<div>
											<p class="text-xs text-muted-foreground">จำนวนผู้ประเมินที่เลือก</p>
											<p class="font-medium">
												{selectedRequestEvaluatorIds(observation.id).length} คน
											</p>
										</div>
									</div>

									<div class="mt-4 grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
										<div class="space-y-2">
											<Label>ผู้ประเมิน</Label>
											<div class="flex min-h-10 flex-wrap items-center gap-2 rounded-md border p-2">
												{#if selectedRequestEvaluators(observation.id).length === 0}
													<span class="text-sm text-muted-foreground">ยังไม่ได้เลือกผู้ประเมิน</span
													>
												{:else}
													{#each selectedRequestEvaluators(observation.id) as evaluator (evaluator.id)}
														<Badge variant="secondary" class="gap-1 pr-1">
															<span>{evaluator.name}</span>
															<button
																type="button"
																class="rounded-sm p-0.5 text-muted-foreground hover:bg-background hover:text-foreground"
																aria-label={`ลบผู้ประเมิน ${evaluator.name}`}
																onclick={() =>
																	removeRequestEvaluatorForRequest(observation.id, evaluator.id)}
															>
																<Trash2 class="h-3 w-3" />
															</button>
														</Badge>
													{/each}
												{/if}
											</div>
											<Popover.Root
												open={requestEvaluatorPickerOpen(observation.id)}
												onOpenChange={(open) => setRequestEvaluatorPickerOpen(observation.id, open)}
											>
												<Popover.Trigger>
													{#snippet child({ props })}
														<Button
															type="button"
															variant="outline"
															role="combobox"
															aria-expanded={requestEvaluatorPickerOpen(observation.id)}
															class="w-full justify-between font-normal sm:w-[320px]"
															disabled={mutationBusy}
															{...props}
														>
															<span class="truncate">เพิ่ม/เลือกผู้ประเมิน</span>
															<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
														</Button>
													{/snippet}
												</Popover.Trigger>
												<Popover.Content class="w-[--bits-popover-trigger-width] p-0">
													<Command.Root>
														<Command.Input placeholder="ค้นหาครูผู้ประเมิน..." />
														<Command.Empty>ไม่พบครู</Command.Empty>
														<Command.List class="max-h-72">
															<Command.Group>
																{#each staffList as staff (staff.id)}
																	<Command.Item
																		value={`${staff.name} ${staff.title ?? ''} ${staff.id}`}
																		onSelect={() =>
																			toggleRequestEvaluatorForRequest(observation.id, staff)}
																	>
																		<Check
																			class={cn(
																				'mr-2 h-4 w-4',
																				selectedRequestEvaluatorIds(observation.id).includes(
																					staff.id
																				)
																					? 'opacity-100'
																					: 'opacity-0'
																			)}
																		/>
																		<span>{staff.name}</span>
																		{#if staff.title}
																			<span class="ml-1 text-xs text-muted-foreground"
																				>({staff.title})</span
																			>
																		{/if}
																	</Command.Item>
																{/each}
															</Command.Group>
														</Command.List>
													</Command.Root>
												</Popover.Content>
											</Popover.Root>
										</div>

										<div class="space-y-2">
											<Label>ส่งกลับคำขอ</Label>
											<Textarea
												value={requestReturnComments[observation.id] ?? ''}
												rows={3}
												placeholder="ระบุเหตุผลส่งกลับ"
												oninput={(event) =>
													setRequestReturnCommentForRequest(
														observation.id,
														(event.currentTarget as HTMLTextAreaElement).value
													)}
											/>
										</div>
									</div>

									<div class="mt-4 flex flex-wrap justify-end gap-2">
										<Button
											size="sm"
											variant="outline"
											href={`/staff/academic/supervision/${observation.id}`}
										>
											<Eye class="h-4 w-4" />
											รายละเอียด
										</Button>
										<LoadingButton
											variant="outline"
											loading={savingAction === `return-request:${observation.id}`}
											loadingLabel="กำลังส่งกลับ..."
											disabled={mutationBusy}
											onclick={() => returnRequest(observation.id)}
										>
											ส่งกลับคำขอ
										</LoadingButton>
										<LoadingButton
											onclick={() => approveRequest(observation.id)}
											loading={savingAction === `approve-request:${observation.id}`}
											loadingLabel="กำลังอนุมัติ..."
											disabled={mutationBusy ||
												selectedRequestEvaluatorIds(observation.id).length === 0}
										>
											อนุมัติและมอบหมาย
										</LoadingButton>
									</div>
								</div>
							{/each}
						</div>
					{/if}
				</Card.Content>
			</Card.Root>
		</Tabs.Content>

		<Tabs.Content value="evaluate" class="space-y-4">
			<Card.Root>
				<Card.Header>
					<Card.Title>รายการที่ได้รับมอบหมายให้ประเมิน</Card.Title>
				</Card.Header>
				<Card.Content class="space-y-4">
					{#if activeAssignedObservations.length === 0}
						<PageState
							title="ยังไม่มีรายการที่ได้รับมอบหมาย"
							description="รายการจะปรากฏเมื่อผู้ดูแลอนุมัติคำขอและมอบหมายให้ประเมิน หรือเมื่อมีงานที่ยังไม่ได้ส่งผล"
						/>
					{:else}
						<div class="space-y-3" data-supervision-assigned-list="cards">
							{#each activeAssignedObservations as observation (observation.id)}
								<div
									class={cn(
										'rounded-md border bg-background p-4 transition',
										evaluationObservationId === observation.id && 'border-primary bg-primary/5'
									)}
								>
									<div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
										<div class="min-w-0 space-y-1">
											<div class="flex flex-wrap items-center gap-2">
												<h3 class="font-semibold">
													{observation.observedDisplayName ?? 'ครูผู้ถูกนิเทศ'}
												</h3>
												<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
											</div>
											<p class="text-sm text-muted-foreground">
												{observationSubjectLabel(observation)} · {observationPeriodLabel(
													observation
												)}
											</p>
										</div>
										<div class="shrink-0 text-sm text-muted-foreground">
											{formatDate(observation.observedAt)}
										</div>
									</div>

									<div class="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-5">
										<div>
											<p class="text-xs text-muted-foreground">นิเทศใคร</p>
											<p class="font-medium">{observation.observedDisplayName ?? '-'}</p>
										</div>
										{#each observationDetailGrid(observation) as detail (detail.label)}
											<div>
												<p class="text-xs text-muted-foreground">{detail.label}</p>
												<p class="font-medium">{detail.value}</p>
											</div>
										{/each}
									</div>

									<div
										class="mt-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between"
									>
										<div class="min-w-0">
											<p class="text-xs text-muted-foreground">ผู้นิเทศร่วม</p>
											<div class="mt-1 flex flex-wrap gap-2">
												{#if observation.evaluators.length === 0}
													<Badge variant="outline">ยังไม่มอบหมาย</Badge>
												{:else}
													{#each observation.evaluators as evaluator (evaluator.id)}
														<Badge
															variant={evaluator.evaluatorUserId === currentUserId
																? 'default'
																: 'secondary'}
														>
															{evaluator.evaluatorDisplayName ?? 'ผู้ประเมิน'}
														</Badge>
													{/each}
												{/if}
											</div>
										</div>
										<div class="flex shrink-0 flex-wrap gap-2">
											<Button
												size="sm"
												variant="outline"
												href={`/staff/academic/supervision/${observation.id}`}
											>
												<Eye class="h-4 w-4" />
												รายละเอียด
											</Button>
											<Button
												type="button"
												size="sm"
												variant={evaluationObservationId === observation.id ? 'default' : 'outline'}
												onclick={() => prepareEvaluationDraft(observation)}
											>
												เปิดแบบประเมิน
											</Button>
										</div>
									</div>
								</div>
							{/each}
						</div>
					{/if}

					{#if submittedAssignedObservations.length > 0}
						<div class="space-y-3 border-t pt-4">
							<div>
								<h3 class="text-sm font-semibold">ประวัติการประเมินที่ส่งแล้ว</h3>
								<p class="text-xs text-muted-foreground">
									รายการที่ส่งผลประเมินแล้วจะเก็บไว้ตรวจสอบย้อนหลัง ไม่แสดงปนกับคิวที่ต้องทำ
								</p>
							</div>

							<div class="space-y-3" data-supervision-submitted-assigned-list="cards">
								{#each submittedAssignedObservations as observation (observation.id)}
									{@const submittedEvaluator = currentUserEvaluator(observation)}
									<div class="rounded-md border bg-muted/20 p-4">
										<div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
											<div class="min-w-0 space-y-1">
												<div class="flex flex-wrap items-center gap-2">
													<h3 class="font-semibold">
														{observation.observedDisplayName ?? 'ครูผู้ถูกนิเทศ'}
													</h3>
													<Badge variant="secondary">ส่งผลแล้ว</Badge>
													<Badge variant="outline">{statusLabel(observation.status)}</Badge>
												</div>
												<p class="text-sm text-muted-foreground">
													{observationSubjectLabel(observation)} · {observationPeriodLabel(
														observation
													)}
												</p>
											</div>
											<div class="shrink-0 text-sm text-muted-foreground">
												ส่งเมื่อ {formatDate(submittedEvaluator?.submittedAt)}
											</div>
										</div>

										<div class="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-5">
											<div>
												<p class="text-xs text-muted-foreground">นิเทศใคร</p>
												<p class="font-medium">{observation.observedDisplayName ?? '-'}</p>
											</div>
											{#each observationDetailGrid(observation) as detail (detail.label)}
												<div>
													<p class="text-xs text-muted-foreground">{detail.label}</p>
													<p class="font-medium">{detail.value}</p>
												</div>
											{/each}
										</div>

										<div
											class="mt-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between"
										>
											<div class="min-w-0">
												<p class="text-xs text-muted-foreground">ผู้นิเทศร่วม</p>
												<div class="mt-1 flex flex-wrap gap-2">
													{#each observation.evaluators as evaluator (evaluator.id)}
														<Badge
															variant={evaluator.evaluatorUserId === currentUserId
																? 'default'
																: 'secondary'}
														>
															{evaluator.evaluatorDisplayName ?? 'ผู้ประเมิน'}
														</Badge>
													{/each}
												</div>
											</div>
											<Button
												size="sm"
												variant="outline"
												href={`/staff/academic/supervision/${observation.id}`}
											>
												<Eye class="h-4 w-4" />
												รายละเอียด
											</Button>
										</div>
									</div>
								{/each}
							</div>
						</div>
					{/if}
				</Card.Content>
			</Card.Root>
		</Tabs.Content>

		<Tabs.Content value="cycles" class="space-y-4">
			<Card.Root>
				<Card.Header class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
					<div>
						<Card.Title>รอบนิเทศ</Card.Title>
						<Card.Description>รอบนิเทศเชื่อมกับปีการศึกษาและภาคเรียนของระบบวิชาการ</Card.Description
						>
					</div>
					{#if canManageSchool}
						<Button onclick={() => (createCycleDialogOpen = true)}>
							<Plus class="mr-2 h-4 w-4" />
							สร้างรอบนิเทศ
						</Button>
					{/if}
				</Card.Header>
				<Card.Content>
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head>รอบนิเทศ</Table.Head>
								<Table.Head>ภาคเรียน</Table.Head>
								<Table.Head>ช่วงเวลา</Table.Head>
								<Table.Head>สถานะ</Table.Head>
								<Table.Head class="text-right">คำสั่ง</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#if cycles.length === 0}
								<Table.Row>
									<Table.Cell colspan={5} class="h-24 text-center text-muted-foreground">
										ยังไม่มีรอบนิเทศ
									</Table.Cell>
								</Table.Row>
							{:else}
								{#each cycles as cycle (cycle.id)}
									<Table.Row>
										<Table.Cell class="font-medium">{cycle.title}</Table.Cell>
										<Table.Cell>
											{cycle.academicSemesterId
												? semesterLabel(cycle.academicSemesterId)
												: `ปี ${cycle.academicYear} / ภาคเรียน ${cycle.semester}`}
										</Table.Cell>
										<Table.Cell
											>{formatDate(cycle.startsAt)} - {formatDate(cycle.endsAt)}</Table.Cell
										>
										<Table.Cell
											><Badge variant="secondary">{statusLabel(cycle.status)}</Badge></Table.Cell
										>
										<Table.Cell class="text-right">
											{#if cycle.status === 'draft'}
												<LoadingButton
													size="sm"
													onclick={() => setCycleStatus(cycle, 'open')}
													loading={savingAction === `cycle-status:${cycle.id}:open`}
													loadingLabel="กำลังเปิด..."
													disabled={mutationBusy}
												>
													เปิดให้จอง
												</LoadingButton>
											{:else if cycle.status === 'open'}
												<LoadingButton
													size="sm"
													variant="outline"
													onclick={() => setCycleStatus(cycle, 'closed')}
													loading={savingAction === `cycle-status:${cycle.id}:closed`}
													loadingLabel="กำลังปิด..."
													disabled={mutationBusy}
												>
													ปิดรอบ
												</LoadingButton>
											{:else if cycle.status === 'closed'}
												<LoadingButton
													size="sm"
													variant="outline"
													onclick={() => setCycleStatus(cycle, 'open')}
													loading={savingAction === `cycle-status:${cycle.id}:open`}
													loadingLabel="กำลังเปิด..."
													disabled={mutationBusy}
												>
													เปิดอีกครั้ง
												</LoadingButton>
											{:else}
												<span class="text-sm text-muted-foreground">-</span>
											{/if}
										</Table.Cell>
									</Table.Row>
								{/each}
							{/if}
						</Table.Body>
					</Table.Root>
				</Card.Content>
			</Card.Root>
		</Tabs.Content>

		<Tabs.Content value="templates" class="space-y-4">
			<Card.Root>
				<Card.Header class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
					<div>
						<Card.Title>แบบประเมิน</Card.Title>
						<Card.Description>แบบประเมินใช้กับรอบนิเทศและขั้นตอนอนุมัติผล</Card.Description>
					</div>
					{#if canManageSchool}
						<Button onclick={openCreateTemplateDialog}>
							<Plus class="mr-2 h-4 w-4" />
							สร้างแบบประเมิน
						</Button>
					{/if}
				</Card.Header>
				<Card.Content>
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head>ชื่อแบบประเมิน</Table.Head>
								<Table.Head>หมวด</Table.Head>
								<Table.Head>ข้อ</Table.Head>
								<Table.Head>ช่วงคะแนน</Table.Head>
								<Table.Head>สถานะ</Table.Head>
								<Table.Head class="text-right">คำสั่ง</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#if templates.length === 0}
								<Table.Row>
									<Table.Cell colspan={6} class="h-24 text-center text-muted-foreground">
										ยังไม่มีแบบประเมินนิเทศ
									</Table.Cell>
								</Table.Row>
							{:else}
								{#each templates as template (template.id)}
									<Table.Row>
										<Table.Cell class="font-medium">{template.title}</Table.Cell>
										<Table.Cell>{template.sections.length}</Table.Cell>
										<Table.Cell>{templateItemCount(template)}</Table.Cell>
										<Table.Cell>{template.ratingMin} - {template.ratingMax}</Table.Cell>
										<Table.Cell>
											<Badge variant="secondary">{templateStatusLabel(template.status)}</Badge>
										</Table.Cell>
										<Table.Cell class="text-right">
											<div class="flex justify-end gap-2">
												<Button
													size="sm"
													variant="outline"
													onclick={() => openTemplatePreviewDialog(template)}
												>
													<Eye class="mr-2 h-4 w-4" />
													ดูตัวอย่าง
												</Button>
												{#if canManageSchool}
													<Button
														size="sm"
														variant="outline"
														onclick={() => openEditTemplateDialog(template)}
													>
														แก้ไข
													</Button>
												{/if}
											</div>
										</Table.Cell>
									</Table.Row>
								{/each}
							{/if}
						</Table.Body>
					</Table.Root>
				</Card.Content>
			</Card.Root>
		</Tabs.Content>

		<Tabs.Content value="reports" class="space-y-4">
			<Card.Root>
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<BarChart3 class="h-5 w-5" />
						รายงานความคืบหน้ารอบนิเทศ
					</Card.Title>
				</Card.Header>
				<Card.Content class="space-y-4">
					<div class="flex flex-col gap-2 md:flex-row">
						<Select.Root type="single" bind:value={progressCycleId}>
							<Select.Trigger class="w-full md:w-[360px]">
								{cycles.find((cycle) => cycle.id === progressCycleId)?.title ?? 'เลือกรอบนิเทศ'}
							</Select.Trigger>
							<Select.Content>
								{#each cycles as cycle (cycle.id)}
									<Select.Item value={cycle.id}>{cycleLabel(cycle)}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						<LoadingButton onclick={loadProgress} loading={saving} loadingLabel="กำลังโหลด...">
							โหลดรายงาน
						</LoadingButton>
					</div>

					{#if progress}
						<div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
							<div class="rounded-md border px-3 py-2">
								<p class="text-xl font-semibold">{progress.totalObservations}</p>
								<p class="text-xs text-muted-foreground">ทั้งหมด</p>
							</div>
							<div class="rounded-md border px-3 py-2">
								<p class="text-xl font-semibold">{progress.completedCount}</p>
								<p class="text-xs text-muted-foreground">เสร็จสิ้น/รับทราบ</p>
							</div>
							<div class="rounded-md border px-3 py-2">
								<p class="text-xl font-semibold">{progress.underReviewCount}</p>
								<p class="text-xs text-muted-foreground">รอตรวจทาน</p>
							</div>
							<div class="rounded-md border px-3 py-2">
								<p class="text-xl font-semibold">{progress.averageRating?.toFixed(2) ?? '-'}</p>
								<p class="text-xs text-muted-foreground">คะแนนเฉลี่ย</p>
							</div>
						</div>
						<div class="space-y-2">
							<div class="flex items-center justify-between text-sm">
								<span>ความคืบหน้ารวม</span>
								<span>{progressPercent}%</span>
							</div>
							<Progress value={progressPercent} />
						</div>
					{/if}

					<div class="space-y-2">
						{#if canApprove}
							<Textarea bind:value={reviewComment} rows={2} placeholder="เหตุผลส่งกลับผลนิเทศ" />
						{/if}
						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head>ครูผู้ถูกนิเทศ</Table.Head>
									<Table.Head>คาบ</Table.Head>
									<Table.Head>สถานะ</Table.Head>
									<Table.Head class="text-right">คำสั่ง</Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#if reviewableObservations.length === 0}
									<Table.Row>
										<Table.Cell colspan={4} class="h-24 text-center text-muted-foreground">
											ยังไม่มีรายการที่ต้องตรวจทาน
										</Table.Cell>
									</Table.Row>
								{:else}
									{#each reviewableObservations as observation (observation.id)}
										<Table.Row>
											<Table.Cell>{observation.observedDisplayName ?? 'ครู'}</Table.Cell>
											<Table.Cell>{observationLessonTitle(observation)}</Table.Cell>
											<Table.Cell>
												<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
											</Table.Cell>
											<Table.Cell class="space-x-2 text-right">
												<Button
													size="sm"
													variant="outline"
													href={`/staff/academic/supervision/${observation.id}`}
												>
													<Eye class="h-4 w-4" />
													รายละเอียด
												</Button>
												{#if canManageRequests}
													<LoadingButton
														size="sm"
														variant="outline"
														onclick={() => submitForReview(observation.id)}
														loading={savingAction === `submit-review:${observation.id}`}
														loadingLabel="กำลังส่ง..."
														disabled={mutationBusy}
													>
														ส่งตรวจทาน
													</LoadingButton>
												{/if}
												{#if canApprove}
													<LoadingButton
														size="sm"
														variant="outline"
														onclick={() => approveResult(observation.id)}
														loading={savingAction === `approve-result:${observation.id}`}
														loadingLabel="กำลังอนุมัติ..."
														disabled={mutationBusy}
													>
														อนุมัติ
													</LoadingButton>
													<LoadingButton
														size="sm"
														variant="outline"
														onclick={() => publishResult(observation.id)}
														loading={savingAction === `publish-result:${observation.id}`}
														loadingLabel="กำลังเผยแพร่..."
														disabled={mutationBusy}
													>
														เผยแพร่
													</LoadingButton>
													<LoadingButton
														size="sm"
														variant="outline"
														onclick={() => returnResult(observation.id)}
														loading={savingAction === `return-result:${observation.id}`}
														loadingLabel="กำลังส่งกลับ..."
														disabled={mutationBusy}
													>
														ส่งกลับ
													</LoadingButton>
												{/if}
												{#if !canManageRequests && !canApprove}
													<span class="text-sm text-muted-foreground">-</span>
												{/if}
											</Table.Cell>
										</Table.Row>
									{/each}
								{/if}
							</Table.Body>
						</Table.Root>
					</div>
				</Card.Content>
			</Card.Root>
		</Tabs.Content>
	</Tabs.Root>
</PageShell>

<Dialog.Root bind:open={evaluationDialogOpen} onOpenChange={setEvaluationDialogOpen}>
	<Dialog.Content class="flex max-h-[92vh] flex-col sm:max-w-5xl">
		<Dialog.Header>
			<Dialog.Title>ทำแบบประเมินนิเทศ</Dialog.Title>
			<Dialog.Description>
				{#if selectedEvaluation && selectedEvaluationTemplate}
					{selectedEvaluationTemplate.title} · {selectedEvaluation.observedDisplayName ??
						'ครูผู้ถูกนิเทศ'}
				{:else}
					เลือกรายการที่ได้รับมอบหมายเพื่อทำแบบประเมิน
				{/if}
			</Dialog.Description>
		</Dialog.Header>

		{#if selectedEvaluation && selectedEvaluationTemplate}
			<div
				class="grid gap-3 rounded-md border bg-muted/20 p-3 text-sm sm:grid-cols-2 lg:grid-cols-4"
			>
				<div>
					<p class="text-xs text-muted-foreground">คะแนนรวม</p>
					<p class="font-semibold">
						{selectedEvaluationDraftSummary.totalScore} / {selectedEvaluationDraftSummary.maxScore}
					</p>
				</div>
				<div>
					<p class="text-xs text-muted-foreground">ร้อยละ</p>
					<p class="font-semibold">
						{selectedEvaluationDraftSummary.percentage === null
							? '-'
							: selectedEvaluationDraftSummary.percentage.toFixed(2)}
					</p>
				</div>
				<div>
					<p class="text-xs text-muted-foreground">ระดับคุณภาพ</p>
					<p class="font-semibold">{selectedEvaluationDraftSummary.qualityLabel}</p>
				</div>
				<div>
					<p class="text-xs text-muted-foreground">ตอบแล้ว</p>
					<p class="font-semibold">
						{selectedEvaluationDraftSummary.answeredRatingCount} /
						{selectedEvaluationDraftSummary.ratingItemCount}
					</p>
				</div>
			</div>

			<div class="grid gap-3 rounded-md border p-3 text-sm sm:grid-cols-2 lg:grid-cols-4">
				<div>
					<p class="text-xs text-muted-foreground">นิเทศใคร</p>
					<p class="font-medium">{selectedEvaluation.observedDisplayName ?? '-'}</p>
				</div>
				{#each observationDetailGrid(selectedEvaluation) as detail (detail.label)}
					<div>
						<p class="text-xs text-muted-foreground">{detail.label}</p>
						<p class="font-medium">{detail.value}</p>
					</div>
				{/each}
			</div>

			<div class="min-h-0 flex-1 space-y-4 overflow-y-auto pr-1">
				{#each selectedEvaluationRubricSections as section (section.localId)}
					{@const progress = sectionRubricProgress(
						section,
						responseDrafts,
						selectedEvaluationTemplate.ratingMax
					)}
					<div class="space-y-3 rounded-md border bg-background p-3">
						<div class="flex flex-wrap items-start justify-between gap-2">
							<div>
								<h4 class="text-sm font-semibold">{section.title}</h4>
								{#if section.description}
									<p class="text-xs text-muted-foreground">{section.description}</p>
								{/if}
							</div>
							<div class="flex flex-wrap gap-2">
								<Badge variant="secondary">
									บังคับ {progress.answeredRequiredCount}/{progress.requiredCount}
								</Badge>
								<Badge variant="outline">
									คะแนน {progress.totalScore}/{progress.maxScore}
								</Badge>
								<Badge variant="outline">
									{progress.percentage === null ? '-' : progress.percentage.toFixed(2)}% ·
									{progress.qualityLabel}
								</Badge>
							</div>
						</div>
						{#each section.items as item (item.localId)}
							<div class="space-y-2 rounded-md border p-3">
								<div class="space-y-1">
									<Label>{item.label}</Label>
									{#if item.description}
										<p class="text-xs text-muted-foreground">{item.description}</p>
									{/if}
								</div>
								{#if item.itemType === 'rating'}
									<div class="flex flex-wrap gap-2">
										{#each ratingScale(selectedEvaluationTemplate.ratingMin, selectedEvaluationTemplate.ratingMax) as score (score)}
											<Button
												type="button"
												size="sm"
												variant={responseDrafts[item.localId]?.ratingScore === String(score)
													? 'default'
													: 'outline'}
												onclick={() => updateDraft(item.localId, { ratingScore: String(score) })}
											>
												{score}
											</Button>
										{/each}
									</div>
								{:else}
									<Textarea
										rows={3}
										value={responseDrafts[item.localId]?.textResponse ?? ''}
										oninput={(event) =>
											updateDraft(item.localId, {
												textResponse: (event.currentTarget as HTMLTextAreaElement).value
											})}
									/>
								{/if}
							</div>
						{/each}
					</div>
				{/each}
			</div>

			<Dialog.Footer>
				<Button
					variant="outline"
					onclick={clearEvaluationDraft}
					disabled={savingEvaluation !== null}
				>
					ปิด
				</Button>
				<LoadingButton
					variant="outline"
					onclick={() => saveEvaluation(false)}
					loading={savingEvaluation === 'draft'}
					loadingLabel="กำลังบันทึก..."
					disabled={savingEvaluation !== null}
				>
					บันทึกร่าง
				</LoadingButton>
				<LoadingButton
					onclick={() => saveEvaluation(true)}
					loading={savingEvaluation === 'submit'}
					loadingLabel="กำลังส่ง..."
					disabled={savingEvaluation !== null}
				>
					ส่งผลประเมิน
				</LoadingButton>
			</Dialog.Footer>
		{:else}
			<PageState
				title="ยังไม่ได้เลือกรายการประเมิน"
				description="เลือกงานนิเทศที่ได้รับมอบหมายก่อนเริ่มทำแบบประเมิน"
			/>
		{/if}
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={createCycleDialogOpen}>
	<Dialog.Content class="max-w-3xl">
		<Dialog.Header>
			<Dialog.Title>สร้างรอบนิเทศ</Dialog.Title>
			<Dialog.Description>
				ผูกรอบนิเทศกับปีการศึกษาและภาคเรียนเดิม เพื่อใช้ร่วมกับตารางสอนและรายงาน
			</Dialog.Description>
		</Dialog.Header>
		<div class="grid gap-4 py-2 lg:grid-cols-2">
			<div class="space-y-2">
				<Label>ปีการศึกษา</Label>
				<Select.Root type="single" bind:value={cycleAcademicYearId}>
					<Select.Trigger class="w-full">{cycleYear?.name ?? 'เลือกปีการศึกษา'}</Select.Trigger>
					<Select.Content>
						{#each academicStructure.years as year (year.id)}
							<Select.Item value={year.id}>{year.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
			<div class="space-y-2">
				<Label>ภาคเรียน</Label>
				<Select.Root type="single" bind:value={cycleForm.academicSemesterId}>
					<Select.Trigger class="w-full">
						{cycleForm.academicSemesterId
							? semesterLabel(cycleForm.academicSemesterId)
							: 'เลือกภาคเรียน'}
					</Select.Trigger>
					<Select.Content>
						{#each cycleSemesters as semester (semester.id)}
							<Select.Item value={semester.id}>
								{semester.name || `ภาคเรียนที่ ${semester.term}`}
							</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
			<div class="space-y-2 lg:col-span-2">
				<Label>ชื่อรอบนิเทศ</Label>
				<Input bind:value={cycleForm.title} placeholder="เช่น นิเทศการสอน ภาคเรียนที่ 1" />
			</div>
			<div class="space-y-2 lg:col-span-2">
				<Label>แบบประเมิน</Label>
				<Select.Root type="single" bind:value={cycleForm.templateId}>
					<Select.Trigger class="w-full">
						{templates.find((template) => template.id === cycleForm.templateId)?.title ??
							'เลือกแบบประเมิน'}
					</Select.Trigger>
					<Select.Content>
						{#each templates as template (template.id)}
							<Select.Item value={template.id}>{template.title}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
			<div class="space-y-2 lg:col-span-2">
				<Label>สถานะรอบ</Label>
				<Select.Root type="single" bind:value={cycleForm.status}>
					<Select.Trigger class="w-full">
						{cycleStatusCreateOptions.find((option) => option.value === cycleForm.status)?.label ??
							'เลือกสถานะรอบ'}
					</Select.Trigger>
					<Select.Content>
						{#each cycleStatusCreateOptions as option (option.value)}
							<Select.Item value={option.value}>
								<span class="flex flex-col items-start">
									<span>{option.label}</span>
									<span class="text-xs text-muted-foreground">{option.description}</span>
								</span>
							</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				<p class="text-xs text-muted-foreground">
					ค่าเริ่มต้นคือเปิดให้จอง ครูจะเห็นรอบนี้เมื่ออยู่ในช่วงเปิดจองและมีสิทธิ์จอง
				</p>
			</div>
			<div class="space-y-2">
				<Label>เปิดจองวันที่</Label>
				<DatePicker bind:value={cycleForm.bookingOpensDate} placeholder="วันเปิดจอง" />
			</div>
			<div class="space-y-2">
				<Label>เวลาเปิดจอง</Label>
				<Input type="time" bind:value={cycleForm.bookingOpensTime} />
			</div>
			<div class="space-y-2">
				<Label>ปิดจองวันที่</Label>
				<DatePicker bind:value={cycleForm.bookingClosesDate} placeholder="วันปิดจอง" />
			</div>
			<div class="space-y-2">
				<Label>เวลาปิดจอง</Label>
				<Input type="time" bind:value={cycleForm.bookingClosesTime} />
			</div>
			<div class="space-y-2">
				<Label>เริ่มรอบวันที่</Label>
				<DatePicker bind:value={cycleForm.startsDate} placeholder="วันเริ่มรอบ" />
			</div>
			<div class="space-y-2">
				<Label>เวลาเริ่ม</Label>
				<Input type="time" bind:value={cycleForm.startsTime} />
			</div>
			<div class="space-y-2">
				<Label>สิ้นสุดรอบวันที่</Label>
				<DatePicker bind:value={cycleForm.endsDate} placeholder="วันสิ้นสุดรอบ" />
			</div>
			<div class="space-y-2">
				<Label>เวลาสิ้นสุด</Label>
				<Input type="time" bind:value={cycleForm.endsTime} />
			</div>
			<div class="space-y-2 lg:col-span-2">
				<Label>รายละเอียด</Label>
				<Textarea bind:value={cycleForm.description} rows={2} placeholder="รายละเอียดเพิ่มเติม" />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (createCycleDialogOpen = false)}>ยกเลิก</Button>
			<LoadingButton
				onclick={createCycle}
				loading={savingAction === 'create-cycle'}
				loadingLabel="กำลังสร้าง..."
			>
				สร้างรอบนิเทศ
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={createTemplateDialogOpen}>
	<Dialog.Content
		class="flex max-h-[92vh] w-[calc(100vw-1rem)] max-w-6xl flex-col gap-0 overflow-hidden p-0 sm:w-[calc(100vw-2rem)]"
	>
		<Dialog.Header class="border-b px-4 py-4 pr-12 text-left sm:px-6">
			<Dialog.Title
				>{editingTemplateId ? 'แก้ไขแบบประเมินนิเทศ' : 'สร้างแบบประเมินนิเทศ'}</Dialog.Title
			>
			<Dialog.Description>
				กำหนดหมวดและหัวข้อประเมินหลายข้อ รองรับแบบฟอร์มนิเทศการสอนจริงของโรงเรียน
			</Dialog.Description>
		</Dialog.Header>
		<div class="min-h-0 flex-1 space-y-4 overflow-y-auto overflow-x-hidden px-4 py-4 sm:px-6">
			<div class="grid min-w-0 gap-4 md:grid-cols-3">
				<div class="min-w-0 space-y-2 md:col-span-3">
					<Label>ชื่อแบบประเมิน</Label>
					<Input bind:value={templateForm.title} placeholder="ชื่อแบบประเมิน" />
				</div>
				<div class="min-w-0 space-y-2">
					<Label>สถานะ</Label>
					<Select.Root type="single" bind:value={templateForm.status}>
						<Select.Trigger class="w-full">
							{templateForm.status === 'active'
								? 'ใช้งาน'
								: templateForm.status === 'archived'
									? 'เก็บถาวร'
									: 'ร่าง'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="draft">ร่าง</Select.Item>
							<Select.Item value="active">ใช้งาน</Select.Item>
							<Select.Item value="archived">เก็บถาวร</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
				<div class="min-w-0 space-y-2">
					<Label>คะแนนต่ำสุด</Label>
					<Input type="number" min="0" bind:value={templateForm.ratingMin} />
				</div>
				<div class="min-w-0 space-y-2">
					<Label>คะแนนสูงสุด</Label>
					<Input type="number" min="1" bind:value={templateForm.ratingMax} />
				</div>
				<div class="min-w-0 space-y-2 md:col-span-3">
					<Label>รายละเอียด</Label>
					<Textarea bind:value={templateForm.description} rows={2} placeholder="รายละเอียด" />
				</div>
			</div>

			<div
				class="flex flex-wrap items-center justify-between gap-2 rounded-md border bg-muted/20 p-3"
			>
				<div>
					<p class="text-sm font-medium">โครงสร้างแบบประเมิน</p>
					<p class="text-xs text-muted-foreground">
						{templateForm.sections.length} หมวด · {templateForm.sections.reduce(
							(sum, section) => sum + section.items.length,
							0
						)}
						ข้อ
					</p>
				</div>
				<div class="flex flex-wrap gap-2">
					<Button type="button" variant="outline" size="sm" onclick={loadPaperTemplatePreset}>
						โหลดแบบฟอร์มนิเทศมาตรฐาน
					</Button>
					<Button type="button" size="sm" onclick={addTemplateSection}>
						<Plus class="mr-2 h-4 w-4" />
						เพิ่มหมวด
					</Button>
				</div>
			</div>

			<div class="space-y-3">
				{#each templateForm.sections as section, sectionIndex (section.localId)}
					<div class="min-w-0 rounded-md border">
						<div class="space-y-3 border-b bg-muted/10 p-3">
							<div class="flex flex-col gap-3 lg:flex-row lg:items-end lg:justify-between">
								<div class="min-w-0 flex-1 space-y-2">
									<Label>ชื่อหมวด</Label>
									<Input
										value={section.title}
										oninput={(event) =>
											updateTemplateSection(section.localId, {
												title: (event.currentTarget as HTMLInputElement).value
											})}
									/>
								</div>
								<div class="flex shrink-0 gap-1">
									<Button
										type="button"
										variant="outline"
										size="icon"
										disabled={sectionIndex === 0}
										onclick={() => moveTemplateSection(section.localId, -1)}
										aria-label="ย้ายหมวดขึ้น"
									>
										<ArrowUp class="h-4 w-4" />
									</Button>
									<Button
										type="button"
										variant="outline"
										size="icon"
										disabled={sectionIndex === templateForm.sections.length - 1}
										onclick={() => moveTemplateSection(section.localId, 1)}
										aria-label="ย้ายหมวดลง"
									>
										<ArrowDown class="h-4 w-4" />
									</Button>
									<Button
										type="button"
										variant="outline"
										size="icon"
										onclick={() => removeTemplateSection(section.localId)}
										aria-label="ลบหมวด"
									>
										<Trash2 class="h-4 w-4" />
									</Button>
								</div>
							</div>
							<div class="space-y-2">
								<Label>คำอธิบายหมวด</Label>
								<Input
									value={section.description}
									placeholder="เว้นว่างได้"
									oninput={(event) =>
										updateTemplateSection(section.localId, {
											description: (event.currentTarget as HTMLInputElement).value
										})}
								/>
							</div>
						</div>

						<div class="space-y-2 p-3">
							{#if section.items.length === 0}
								<PageState
									title="หมวดนี้ยังไม่มีหัวข้อ"
									description="เพิ่มหัวข้อแบบคะแนนหรือข้อเสนอแนะ"
								/>
							{:else}
								{#each section.items as item, itemIndex (item.localId)}
									<div class="min-w-0 rounded-md border p-3">
										<div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
											<div class="flex min-w-0 flex-wrap items-center gap-3">
												<Badge variant="secondary">
													{item.itemType === 'rating' ? 'คะแนน' : 'ข้อความ'}
												</Badge>
												<div class="flex items-center gap-2">
													<Checkbox
														checked={item.required}
														onCheckedChange={(checked) =>
															updateTemplateItem(section.localId, item.localId, {
																required: !!checked
															})}
														aria-label="บังคับตอบ"
													/>
													<span class="text-xs text-muted-foreground">บังคับตอบ</span>
												</div>
											</div>
											<div class="flex shrink-0 gap-1">
												<Button
													type="button"
													variant="outline"
													size="icon"
													disabled={itemIndex === 0}
													onclick={() => moveTemplateItem(section.localId, item.localId, -1)}
													aria-label="ย้ายหัวข้อขึ้น"
												>
													<ArrowUp class="h-4 w-4" />
												</Button>
												<Button
													type="button"
													variant="outline"
													size="icon"
													disabled={itemIndex === section.items.length - 1}
													onclick={() => moveTemplateItem(section.localId, item.localId, 1)}
													aria-label="ย้ายหัวข้อลง"
												>
													<ArrowDown class="h-4 w-4" />
												</Button>
												<Button
													type="button"
													variant="outline"
													size="icon"
													onclick={() => removeTemplateItem(section.localId, item.localId)}
													aria-label="ลบหัวข้อ"
												>
													<Trash2 class="h-4 w-4" />
												</Button>
											</div>
										</div>
										<div class="mt-3 grid min-w-0 gap-3 lg:grid-cols-2">
											<div class="min-w-0 space-y-2">
												<Label>หัวข้อประเมิน</Label>
												<Input
													value={item.label}
													oninput={(event) =>
														updateTemplateItem(section.localId, item.localId, {
															label: (event.currentTarget as HTMLInputElement).value
														})}
												/>
											</div>
											<div class="min-w-0 space-y-2">
												<Label>คำอธิบาย</Label>
												<Input
													value={item.description}
													placeholder="เว้นว่างได้"
													oninput={(event) =>
														updateTemplateItem(section.localId, item.localId, {
															description: (event.currentTarget as HTMLInputElement).value
														})}
												/>
											</div>
										</div>
									</div>
								{/each}
							{/if}

							<div class="flex flex-wrap gap-2">
								<Button
									type="button"
									variant="outline"
									size="sm"
									onclick={() => addTemplateItem(section.localId, 'rating')}
								>
									<Plus class="mr-2 h-4 w-4" />
									เพิ่มข้อคะแนน
								</Button>
								<Button
									type="button"
									variant="outline"
									size="sm"
									onclick={() => addTemplateItem(section.localId, 'text')}
								>
									<Plus class="mr-2 h-4 w-4" />
									เพิ่มข้อเสนอแนะ
								</Button>
							</div>
						</div>
					</div>
				{/each}
			</div>
		</div>
		<Dialog.Footer class="border-t px-4 py-4 sm:px-6">
			<Button variant="outline" onclick={() => (createTemplateDialogOpen = false)}>ยกเลิก</Button>
			<LoadingButton
				onclick={createTemplate}
				loading={savingTemplate}
				loadingLabel={editingTemplateId ? 'กำลังบันทึก...' : 'กำลังสร้าง...'}
			>
				{editingTemplateId ? 'บันทึกแบบประเมิน' : 'สร้างแบบประเมิน'}
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={previewTemplateDialogOpen} onOpenChange={setTemplatePreviewDialogOpen}>
	<Dialog.Content
		class="flex max-h-[92vh] w-[calc(100vw-1rem)] max-w-5xl flex-col gap-0 overflow-hidden p-0 sm:w-[calc(100vw-2rem)]"
	>
		<Dialog.Header class="border-b px-4 py-4 pr-12 text-left sm:px-6">
			<Dialog.Title>ตัวอย่างแบบประเมินนิเทศ</Dialog.Title>
			<Dialog.Description>
				ตรวจรูปแบบการแสดงผลของแบบประเมินแบบอ่านอย่างเดียว ก่อนนำไปใช้กับรอบนิเทศ
			</Dialog.Description>
		</Dialog.Header>

		<div class="min-h-0 flex-1 overflow-y-auto overflow-x-hidden bg-muted/30 px-3 py-4 sm:px-6">
			{#if previewTemplate}
				<div
					class="mx-auto max-w-4xl rounded-md border bg-background p-4 shadow-sm sm:p-6"
					data-template-preview-mode="readonly"
				>
					<div class="space-y-3 text-center">
						<div
							class="mx-auto flex h-10 w-10 items-center justify-center rounded-md border bg-muted/40"
						>
							<FileSignature class="h-5 w-5" />
						</div>
						<div>
							<h2 class="text-xl font-semibold">{previewTemplate.title}</h2>
							{#if previewTemplate.description}
								<p class="mt-1 text-sm text-muted-foreground">{previewTemplate.description}</p>
							{/if}
						</div>
						<div class="flex flex-wrap justify-center gap-2">
							<Badge variant="secondary">{templateStatusLabel(previewTemplate.status)}</Badge>
							<Badge variant="outline">{previewTemplate.sections.length} หมวด</Badge>
							<Badge variant="outline">{templateItemCount(previewTemplate)} ข้อ</Badge>
							<Badge variant="outline">
								คะแนน {previewTemplate.ratingMin} - {previewTemplate.ratingMax}
							</Badge>
						</div>
					</div>

					<div class="mt-6 rounded-md border">
						<Table.Root class="table-fixed">
							<Table.Header>
								<Table.Row>
									<Table.Head class="w-[42%] whitespace-normal">หัวข้อการประเมิน</Table.Head>
									{#each templateRatingColumns(previewTemplate) as score (score)}
										<Table.Head class="w-10 px-1 text-center text-xs">{score}</Table.Head>
									{/each}
									<Table.Head class="w-[24%] whitespace-normal text-center text-xs">
										ข้อเสนอแนะ
									</Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each previewTemplate.sections as section (section.id)}
									<Table.Row>
										<Table.Cell
											colspan={templateRatingColumns(previewTemplate).length + 2}
											class="whitespace-normal bg-muted/30 font-medium"
										>
											<div>{section.title}</div>
											{#if section.description}
												<div class="mt-1 text-xs font-normal text-muted-foreground">
													{section.description}
												</div>
											{/if}
										</Table.Cell>
									</Table.Row>
									{#if section.items.length === 0}
										<Table.Row>
											<Table.Cell
												colspan={templateRatingColumns(previewTemplate).length + 2}
												class="whitespace-normal text-center text-sm text-muted-foreground"
											>
												ยังไม่มีหัวข้อในหมวดนี้
											</Table.Cell>
										</Table.Row>
									{:else}
										{#each section.items as item (item.id)}
											<Table.Row>
												<Table.Cell class="whitespace-normal align-top">
													<div class="font-medium leading-snug">{item.label}</div>
													{#if item.description}
														<div class="mt-1 text-xs text-muted-foreground">
															{item.description}
														</div>
													{/if}
													{#if item.required}
														<div class="mt-1 text-[11px] text-muted-foreground">บังคับตอบ</div>
													{/if}
												</Table.Cell>
												{#if item.itemType === 'rating'}
													{#each templateRatingColumns(previewTemplate) as score (score)}
														<Table.Cell class="px-1 text-center align-top">
															<span
																class="mx-auto block h-4 w-4 rounded border"
																aria-label={`ช่องคะแนน ${score} สำหรับ ${item.label}`}
															></span>
														</Table.Cell>
													{/each}
													<Table.Cell
														class="whitespace-normal align-top text-xs text-muted-foreground"
													>
														พื้นที่ข้อเสนอแนะ
													</Table.Cell>
												{:else}
													<Table.Cell
														colspan={templateRatingColumns(previewTemplate).length}
														class="whitespace-normal text-center text-xs text-muted-foreground"
													>
														คำตอบแบบข้อความ
													</Table.Cell>
													<Table.Cell
														class="whitespace-normal align-top text-xs text-muted-foreground"
													>
														พื้นที่ตอบข้อความ
													</Table.Cell>
												{/if}
											</Table.Row>
										{/each}
									{/if}
								{/each}
							</Table.Body>
						</Table.Root>
					</div>

					<div class="mt-6 grid gap-3 sm:grid-cols-2">
						<div class="rounded-md border p-3">
							<p class="text-xs text-muted-foreground">ผู้รับการนิเทศ</p>
							<div class="mt-8 border-t pt-2 text-center text-xs text-muted-foreground">ลงชื่อ</div>
						</div>
						<div class="rounded-md border p-3">
							<p class="text-xs text-muted-foreground">ผู้นิเทศ</p>
							<div class="mt-8 border-t pt-2 text-center text-xs text-muted-foreground">ลงชื่อ</div>
						</div>
					</div>
				</div>
			{:else}
				<PageState title="ไม่พบแบบประเมิน" description="เลือกแบบประเมินจากรายการอีกครั้ง" />
			{/if}
		</div>

		<Dialog.Footer class="border-t px-4 py-4 sm:px-6">
			<Button variant="outline" onclick={() => setTemplatePreviewDialogOpen(false)}>ปิด</Button>
			{#if previewTemplate && canManageSchool}
				<Button
					onclick={() => {
						const template = previewTemplate;
						setTemplatePreviewDialogOpen(false);
						openEditTemplateDialog(template);
					}}
				>
					แก้ไขแบบประเมิน
				</Button>
			{/if}
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
