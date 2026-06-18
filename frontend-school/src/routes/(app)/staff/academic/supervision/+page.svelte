<script lang="ts">
	import { onMount } from 'svelte';
	import {
		BarChart3,
		BookOpenCheck,
		Check,
		ChevronsUpDown,
		FileSignature,
		Loader2,
		Plus,
		RefreshCw,
		Send,
		Settings2,
		UserCheck
	} from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { getAcademicStructure, type AcademicStructureData } from '$lib/api/academic';
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
		type CreateSupervisionCycleRequest,
		type CreateSupervisionTemplateRequest,
		type SaveEvaluationRequest,
		type SupervisionCycle,
		type SupervisionCycleStatus,
		type SupervisionCycleProgress,
		type SupervisionObservation,
		type SupervisionObservationStatus,
		type SupervisionTemplate
	} from '$lib/api/supervision';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import { cn } from '$lib/utils';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import * as Alert from '$lib/components/ui/alert';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Command from '$lib/components/ui/command';
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

	type ResponseDraft = {
		ratingScore: string;
		textResponse: string;
	};

	const timetableGridDays = [
		{ code: 'MON', label: 'จันทร์' },
		{ code: 'TUE', label: 'อังคาร' },
		{ code: 'WED', label: 'พุธ' },
		{ code: 'THU', label: 'พฤหัส' },
		{ code: 'FRI', label: 'ศุกร์' },
		{ code: 'SAT', label: 'เสาร์' },
		{ code: 'SUN', label: 'อาทิตย์' }
	] as const;

	type TimetableGridDayCode = (typeof timetableGridDays)[number]['code'];
	type TimetablePeriodRow = {
		key: string;
		label: string;
		timeLabel: string;
		sort: number;
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

	let { data } = $props();

	let loading = $state(true);
	let loadingTimetable = $state(false);
	let saving = $state(false);
	let activeTab = $state('mine');
	let cycles = $state<SupervisionCycle[]>([]);
	let templates = $state<SupervisionTemplate[]>([]);
	let observations = $state<SupervisionObservation[]>([]);
	let timetableEntries = $state<TimetableEntry[]>([]);
	let staffList = $state<StaffLookupItem[]>([]);
	let academicStructure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let selectedCycleId = $state('');
	let selectedTimetableEntryId = $state('');
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
	let requestReturnComment = $state('');
	let approvalObservationId = $state('');
	let approvalEvaluatorId = $state('');
	let evaluatorPickerOpen = $state(false);
	let evaluationObservationId = $state('');
	let responseDrafts = $state<{ [itemId: string]: ResponseDraft }>({});
	let acknowledgeComment = $state('');
	let reviewComment = $state('');
	let progressCycleId = $state('');
	let progress = $state<SupervisionCycleProgress | null>(null);
	let createCycleDialogOpen = $state(false);
	let createTemplateDialogOpen = $state(false);
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
	let templateForm = $state({
		title: '',
		description: '',
		ratingMin: 1,
		ratingMax: 5,
		ratingLabel: 'การจัดกิจกรรมการเรียนรู้เหมาะสม',
		textLabel: 'ข้อเสนอแนะเพิ่มเติม'
	});

	const currentUserId = $derived($authStore.user?.id ?? '');
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
	const requestedObservations = $derived(
		observations.filter((observation) => observation.status === 'requested')
	);
	const selectedCycleDetail = $derived(
		cycles.find((cycle) => cycle.id === selectedCycleId) ?? null
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
	const selectedApprovalObservation = $derived(
		observations.find((observation) => observation.id === approvalObservationId) ?? null
	);
	const selectedEvaluator = $derived(
		staffList.find((staff) => staff.id === approvalEvaluatorId) ?? null
	);
	const selectedEvaluation = $derived(
		observations.find((observation) => observation.id === evaluationObservationId) ?? null
	);
	const selectedEvaluationTemplate = $derived(
		selectedEvaluation
			? (templates.find((template) => template.id === selectedEvaluation.templateId) ?? null)
			: null
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
		if (!selectedTimetableEntryId) return;
		if (
			!timetableEntriesForSelectedCycle().some((entry) => entry.id === selectedTimetableEntryId)
		) {
			selectedTimetableEntryId = '';
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

	function timetableLabel(entry: TimetableEntry): string {
		const title = entry.subject_name_th || entry.title || entry.subject_code || 'คาบสอน';
		const period = entry.period_name ? ` ${entry.period_name}` : '';
		const room = entry.room_code ? ` ห้อง ${entry.room_code}` : '';
		const classroom = entry.classroom_name ? ` ${entry.classroom_name}` : '';
		return `${entry.day_of_week}${period} - ${title}${classroom}${room}`;
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
		const periodNumber = entry.period_name?.match(/\d+/)?.[0];
		if (periodNumber) return Number(periodNumber);
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

	function timetableEntryFor(
		day: TimetableGridDayCode,
		row: TimetablePeriodRow
	): TimetableEntry | null {
		return (
			timetableEntriesForSelectedCycle().find(
				(entry) => entry.day_of_week === day && timetablePeriodKey(entry) === row.key
			) ?? null
		);
	}

	function selectTimetableEntry(entry: TimetableEntry) {
		if (!canRequest) return;
		selectedTimetableEntryId = entry.id;
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

	async function createBookingRequest() {
		if (!canRequest) return;
		if (!selectedCycleId) {
			toast.error('เลือกรอบนิเทศก่อน');
			return;
		}

		if (!manualMode && !selectedTimetableEntryId) {
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

		saving = true;
		try {
			const response = await requestSupervisionObservation({
				cycleId: selectedCycleId,
				timetableEntryId: manualMode ? null : selectedTimetableEntryId,
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
			if (!response.success) throw new Error(response.error || 'ส่งคำขอไม่สำเร็จ');
			toast.success('ส่งคำขอจองนิเทศแล้ว');
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ส่งคำขอไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	function selectEvaluator(staff: StaffLookupItem) {
		if (!canManageRequests) return;
		approvalEvaluatorId = staff.id;
		evaluatorPickerOpen = false;
	}

	async function approveRequest() {
		if (!canManageRequests) return;
		if (!approvalObservationId || !approvalEvaluatorId) {
			toast.error('เลือกรายการและผู้ประเมินก่อน');
			return;
		}

		saving = true;
		try {
			const response = await approveSupervisionObservationRequest(approvalObservationId, {
				evaluators: [{ evaluatorUserId: approvalEvaluatorId, isRequired: true }]
			});
			if (!response.success) throw new Error(response.error || 'อนุมัติคำขอไม่สำเร็จ');
			toast.success('อนุมัติคำขอและมอบหมายผู้ประเมินแล้ว');
			approvalObservationId = '';
			approvalEvaluatorId = '';
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'อนุมัติคำขอไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function returnRequest(id: string) {
		if (!canManageRequests) return;
		saving = true;
		try {
			const response = await returnSupervisionObservationRequest(id, {
				comment: requestReturnComment || null
			});
			if (!response.success) throw new Error(response.error || 'ส่งกลับคำขอไม่สำเร็จ');
			toast.success('ส่งกลับคำขอแล้ว');
			requestReturnComment = '';
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ส่งกลับคำขอไม่สำเร็จ');
		} finally {
			saving = false;
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
		for (const section of selectedEvaluationTemplate?.sections ?? []) {
			for (const item of section.items) {
				const draft = responseDrafts[item.id] ?? { ratingScore: '', textResponse: '' };
				if (item.itemType === 'rating') {
					responses.push({
						templateItemId: item.id,
						ratingScore: draft.ratingScore ? Number(draft.ratingScore) : null,
						textResponse: null
					});
				} else {
					responses.push({
						templateItemId: item.id,
						ratingScore: null,
						textResponse: draft.textResponse || null
					});
				}
			}
		}
		return { responses };
	}

	async function saveEvaluation(submit = false) {
		if (!canEvaluate) return;
		if (!evaluationObservationId) {
			toast.error('เลือกรายการประเมินก่อน');
			return;
		}

		saving = true;
		try {
			const payload = evaluationPayload();
			const response = submit
				? await submitMySupervisionEvaluation(evaluationObservationId, payload)
				: await saveMySupervisionEvaluation(evaluationObservationId, payload);
			if (!response.success) throw new Error(response.error || 'บันทึกผลประเมินไม่สำเร็จ');
			toast.success(submit ? 'ส่งผลประเมินแล้ว' : 'บันทึกแบบร่างแล้ว');
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกผลประเมินไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function submitForReview(id: string) {
		if (!canManageRequests) return;
		saving = true;
		try {
			const response = await submitSupervisionObservationForReview(id);
			if (!response.success) throw new Error(response.error || 'ส่งตรวจทานไม่สำเร็จ');
			toast.success('ส่งตรวจทานแล้ว');
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ส่งตรวจทานไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function approveResult(id: string) {
		if (!canApprove) return;
		saving = true;
		try {
			const response = await approveSupervisionObservation(id);
			if (!response.success) throw new Error(response.error || 'อนุมัติผลไม่สำเร็จ');
			toast.success('อนุมัติผลนิเทศแล้ว');
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'อนุมัติผลไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function publishResult(id: string) {
		if (!canApprove) return;
		saving = true;
		try {
			const response = await publishSupervisionObservation(id);
			if (!response.success) throw new Error(response.error || 'เผยแพร่ผลไม่สำเร็จ');
			toast.success('เผยแพร่ผลนิเทศแล้ว');
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'เผยแพร่ผลไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function returnResult(id: string) {
		if (!canApprove) return;
		saving = true;
		try {
			const response = await returnSupervisionObservation(id, { comment: reviewComment || null });
			if (!response.success) throw new Error(response.error || 'ส่งกลับผลไม่สำเร็จ');
			toast.success('ส่งกลับผลนิเทศแล้ว');
			reviewComment = '';
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ส่งกลับผลไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function acknowledgeResult(id: string) {
		saving = true;
		try {
			const response = await acknowledgeSupervisionObservation(id, {
				comment: acknowledgeComment || null
			});
			if (!response.success) throw new Error(response.error || 'รับทราบผลไม่สำเร็จ');
			toast.success('รับทราบผลนิเทศแล้ว');
			acknowledgeComment = '';
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'รับทราบผลไม่สำเร็จ');
		} finally {
			saving = false;
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

		saving = true;
		try {
			const response = await createSupervisionCycle(payload);
			if (!response.success) throw new Error(response.error || 'สร้างรอบนิเทศไม่สำเร็จ');
			toast.success('สร้างรอบนิเทศแล้ว');
			cycleForm.title = '';
			cycleForm.description = '';
			createCycleDialogOpen = false;
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'สร้างรอบนิเทศไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function setCycleStatus(cycle: SupervisionCycle, status: SupervisionCycleStatus) {
		if (!canManageSchool) return;
		if (cycle.status === status) return;

		saving = true;
		try {
			const response = await updateSupervisionCycle(cycle.id, { status });
			if (!response.success) throw new Error(response.error || 'เปลี่ยนสถานะรอบนิเทศไม่สำเร็จ');
			toast.success(`เปลี่ยนสถานะรอบนิเทศเป็น${statusLabel(status)}แล้ว`);
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'เปลี่ยนสถานะรอบนิเทศไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function createTemplate() {
		if (!canManageSchool) return;
		if (!templateForm.title || !templateForm.ratingLabel || !templateForm.textLabel) {
			toast.error('กรอกชื่อแบบประเมินและหัวข้อประเมินให้ครบ');
			return;
		}

		const payload: CreateSupervisionTemplateRequest = {
			title: templateForm.title,
			description: templateForm.description || null,
			status: 'draft',
			ratingMin: Number(templateForm.ratingMin),
			ratingMax: Number(templateForm.ratingMax),
			sections: [
				{
					title: 'การจัดการเรียนรู้',
					sortOrder: 1,
					items: [
						{
							label: templateForm.ratingLabel,
							itemType: 'rating',
							required: true,
							sortOrder: 1
						},
						{
							label: templateForm.textLabel,
							itemType: 'text',
							required: false,
							sortOrder: 2
						}
					]
				}
			],
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

		saving = true;
		try {
			const response = await createSupervisionTemplate(payload);
			if (!response.success) throw new Error(response.error || 'สร้างแบบประเมินไม่สำเร็จ');
			toast.success('สร้างแบบประเมินนิเทศแล้ว');
			templateForm.title = '';
			templateForm.description = '';
			createTemplateDialogOpen = false;
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'สร้างแบบประเมินไม่สำเร็จ');
		} finally {
			saving = false;
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
		<Button variant="outline" size="sm" onclick={refreshAll} disabled={loading || saving}>
			<RefreshCw class={cn('mr-2 h-4 w-4', loading && 'animate-spin')} />
			รีเฟรช
		</Button>
		{#if canManageSchool}
			<Button size="sm" onclick={() => (createCycleDialogOpen = true)}>
				<Plus class="mr-2 h-4 w-4" />
				สร้างรอบนิเทศ
			</Button>
			<Button size="sm" variant="outline" onclick={() => (createTemplateDialogOpen = true)}>
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
			<p class="text-lg font-semibold">{assignedObservations.length}</p>
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
						<div class="grid gap-4 lg:grid-cols-2">
							<div class="space-y-2">
								<Label>รอบนิเทศ</Label>
								<Select.Root type="single" bind:value={selectedCycleId}>
									<Select.Trigger class="w-full">
										{openCycles.find((cycle) => cycle.id === selectedCycleId)?.title ??
											'เลือกรอบนิเทศ'}
									</Select.Trigger>
									<Select.Content>
										{#each openCycles as cycle (cycle.id)}
											<Select.Item value={cycle.id}>{cycleLabel(cycle)}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
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
								<Label>คาบจากตารางสอน</Label>
								{#if selectedCycleDetail?.academicSemesterId}
									<p class="text-xs text-muted-foreground">
										แสดงคาบสอนจาก {semesterLabel(selectedCycleDetail.academicSemesterId)}
									</p>
								{/if}
								{#if loadingTimetable}
									<Alert.Root>
										<Loader2 class="h-4 w-4 animate-spin" />
										<Alert.Title>กำลังโหลดตารางสอน</Alert.Title>
										<Alert.Description>ระบบกำลังโหลดคาบสอนตามภาคเรียนของรอบนิเทศ</Alert.Description>
									</Alert.Root>
								{:else if timetableEntriesForSelectedCycle().length === 0}
									<Alert.Root>
										<Alert.Title>ไม่พบคาบสอนในภาคเรียนนี้</Alert.Title>
										<Alert.Description>
											ตรวจสอบตารางสอนของครู หรือใช้คาบกำหนดเองเมื่อจำเป็น
										</Alert.Description>
									</Alert.Root>
								{:else}
									<div class="grid gap-2 md:hidden">
										{#each timetableEntriesForSelectedCycle() as entry (entry.id)}
											<button
												type="button"
												class={cn(
													'rounded-md border p-3 text-left transition hover:bg-muted/60',
													selectedTimetableEntryId === entry.id && 'border-primary bg-primary/10'
												)}
												onclick={() => selectTimetableEntry(entry)}
											>
												<div class="flex items-center justify-between gap-2">
													<span class="font-medium">{timetableEntryTitle(entry)}</span>
													<Badge variant="secondary">{entry.day_of_week}</Badge>
												</div>
												<p class="text-sm text-muted-foreground">
													{entry.period_name ?? 'คาบสอน'}
													{timetableTimeLabel(entry)}
												</p>
												<p class="text-xs text-muted-foreground">
													{entry.classroom_name ?? '-'}
													{entry.room_code ? `ห้อง ${entry.room_code}` : ''}
												</p>
											</button>
										{/each}
									</div>
									<div class="hidden rounded-md border md:block">
										<Table.Root>
											<Table.Header>
												<Table.Row>
													<Table.Head class="w-[120px]">คาบ</Table.Head>
													{#each timetableGridDays as day (day.code)}
														<Table.Head class="min-w-[150px] text-center">{day.label}</Table.Head>
													{/each}
												</Table.Row>
											</Table.Header>
											<Table.Body>
												{#each timetablePeriodRows() as row (row.key)}
													<Table.Row>
														<Table.Cell class="bg-muted/30 align-top">
															<div class="font-medium">{row.label}</div>
															{#if row.timeLabel}
																<div class="text-xs text-muted-foreground">{row.timeLabel}</div>
															{/if}
														</Table.Cell>
														{#each timetableGridDays as day (day.code)}
															{@const entry = timetableEntryFor(day.code, row)}
															<Table.Cell class="min-w-[150px] p-1 align-top">
																{#if entry}
																	<button
																		type="button"
																		class={cn(
																			'min-h-20 w-full rounded-md border p-2 text-left transition hover:border-primary hover:bg-primary/5',
																			selectedTimetableEntryId === entry.id &&
																				'border-primary bg-primary/10 shadow-sm'
																		)}
																		onclick={() => selectTimetableEntry(entry)}
																	>
																		<div class="text-sm font-medium leading-snug">
																			{timetableEntryTitle(entry)}
																		</div>
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
										เลือกแล้ว: {timetableLabel(selectedTimetableEntry)}
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

						<Button onclick={createBookingRequest} disabled={saving || loading}>
							<Send class="mr-2 h-4 w-4" />
							ส่งคำขอจอง
						</Button>
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
						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head>คาบ</Table.Head>
									<Table.Head>วันที่</Table.Head>
									<Table.Head>สถานะ</Table.Head>
									<Table.Head class="text-right">การรับทราบ</Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each myObservations as observation (observation.id)}
									<Table.Row>
										<Table.Cell class="font-medium"
											>{observationLessonTitle(observation)}</Table.Cell
										>
										<Table.Cell>
											{formatDate(
												observation.lessonSnapshot.observedAt ??
													observation.manualLesson?.observedAt
											)}
										</Table.Cell>
										<Table.Cell>
											<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
										</Table.Cell>
										<Table.Cell class="text-right">
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
															<Button
																onclick={() => acknowledgeResult(observation.id)}
																disabled={saving}
															>
																ยืนยันรับทราบ
															</Button>
														</Dialog.Footer>
													</Dialog.Content>
												</Dialog.Root>
											{:else}
												<span class="text-sm text-muted-foreground">-</span>
											{/if}
										</Table.Cell>
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
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
					<Card.Description>เลือกคำขอและมอบหมายผู้ประเมินก่อนอนุมัติ</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-4">
					{#if requestedObservations.length === 0}
						<PageState
							title="ไม่มีคำขอจองที่รออนุมัติ"
							description="เมื่อครูส่งคำขอจอง รายการจะปรากฏในส่วนนี้"
						/>
					{:else}
						<div class="grid gap-3 lg:grid-cols-[1fr_280px_auto]">
							<Select.Root type="single" bind:value={approvalObservationId}>
								<Select.Trigger class="w-full">
									{selectedApprovalObservation
										? `${selectedApprovalObservation.observedDisplayName ?? 'ครู'} - ${observationLessonTitle(selectedApprovalObservation)}`
										: 'เลือกรายการคำขอ'}
								</Select.Trigger>
								<Select.Content>
									{#each requestedObservations as observation (observation.id)}
										<Select.Item value={observation.id}>
											{observation.observedDisplayName ?? 'ครู'} - {observationLessonTitle(
												observation
											)}
										</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>

							<Popover.Root bind:open={evaluatorPickerOpen}>
								<Popover.Trigger>
									{#snippet child({ props })}
										<Button
											variant="outline"
											role="combobox"
											aria-expanded={evaluatorPickerOpen}
											class="w-full justify-between font-normal"
											{...props}
										>
											<span class={cn('truncate', !selectedEvaluator && 'text-muted-foreground')}>
												{selectedEvaluator?.name ?? 'เลือกผู้ประเมิน'}
											</span>
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
													<Command.Item value={staff.name} onSelect={() => selectEvaluator(staff)}>
														<Check
															class={cn(
																'mr-2 h-4 w-4',
																approvalEvaluatorId === staff.id ? 'opacity-100' : 'opacity-0'
															)}
														/>
														<span>{staff.name}</span>
														{#if staff.title}
															<span class="ml-1 text-xs text-muted-foreground">({staff.title})</span
															>
														{/if}
													</Command.Item>
												{/each}
											</Command.Group>
										</Command.List>
									</Command.Root>
								</Popover.Content>
							</Popover.Root>

							<Button onclick={approveRequest} disabled={saving}>อนุมัติและมอบหมาย</Button>
						</div>
						<div class="space-y-2 rounded-md border p-3">
							<Label>ส่งกลับคำขอ</Label>
							<Textarea
								bind:value={requestReturnComment}
								rows={2}
								placeholder="ระบุเหตุผลส่งกลับ"
							/>
							<Button
								variant="outline"
								size="sm"
								disabled={!approvalObservationId || saving}
								onclick={() => returnRequest(approvalObservationId)}
							>
								ส่งกลับคำขอ
							</Button>
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
					{#if assignedObservations.length === 0}
						<PageState
							title="ยังไม่มีรายการที่ได้รับมอบหมาย"
							description="รายการจะปรากฏเมื่อผู้ดูแลอนุมัติคำขอและมอบหมายให้ประเมิน"
						/>
					{:else}
						<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
							{#each assignedObservations as observation (observation.id)}
								<button
									type="button"
									class={cn(
										'rounded-md border p-3 text-left transition hover:bg-muted/40',
										evaluationObservationId === observation.id && 'border-primary bg-primary/5'
									)}
									onclick={() => prepareEvaluationDraft(observation)}
								>
									<div class="font-medium">
										{observation.observedDisplayName ?? 'ครูผู้ถูกนิเทศ'}
									</div>
									<div class="text-sm text-muted-foreground">
										{observationLessonTitle(observation)}
									</div>
									<Badge class="mt-2" variant="secondary">{statusLabel(observation.status)}</Badge>
								</button>
							{/each}
						</div>
					{/if}

					{#if selectedEvaluation && selectedEvaluationTemplate}
						<div class="space-y-4 rounded-md border p-4">
							<div>
								<h3 class="font-semibold">{selectedEvaluationTemplate.title}</h3>
								<p class="text-sm text-muted-foreground">
									{selectedEvaluation.observedDisplayName ?? 'ครูผู้ถูกนิเทศ'}
								</p>
							</div>
							{#each selectedEvaluationTemplate.sections as section (section.id)}
								<div class="space-y-3">
									<h4 class="text-sm font-semibold">{section.title}</h4>
									{#each section.items as item (item.id)}
										<div class="space-y-2 rounded-md border p-3">
											<Label>{item.label}</Label>
											{#if item.itemType === 'rating'}
												<Input
													type="number"
													min={selectedEvaluationTemplate.ratingMin}
													max={selectedEvaluationTemplate.ratingMax}
													bind:value={responseDrafts[item.id].ratingScore}
													oninput={(event) =>
														updateDraft(item.id, {
															ratingScore: (event.currentTarget as HTMLInputElement).value
														})}
												/>
											{:else}
												<Textarea
													rows={3}
													bind:value={responseDrafts[item.id].textResponse}
													oninput={(event) =>
														updateDraft(item.id, {
															textResponse: (event.currentTarget as HTMLTextAreaElement).value
														})}
												/>
											{/if}
										</div>
									{/each}
								</div>
							{/each}
							<div class="flex flex-wrap gap-2">
								<Button variant="outline" onclick={() => saveEvaluation(false)} disabled={saving}>
									บันทึกร่าง
								</Button>
								<Button onclick={() => saveEvaluation(true)} disabled={saving}>ส่งผลประเมิน</Button>
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
												<Button
													size="sm"
													onclick={() => setCycleStatus(cycle, 'open')}
													disabled={saving}
												>
													เปิดให้จอง
												</Button>
											{:else if cycle.status === 'open'}
												<Button
													size="sm"
													variant="outline"
													onclick={() => setCycleStatus(cycle, 'closed')}
													disabled={saving}
												>
													ปิดรอบ
												</Button>
											{:else if cycle.status === 'closed'}
												<Button
													size="sm"
													variant="outline"
													onclick={() => setCycleStatus(cycle, 'open')}
													disabled={saving}
												>
													เปิดอีกครั้ง
												</Button>
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
						<Button onclick={() => (createTemplateDialogOpen = true)}>
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
								<Table.Head>ช่วงคะแนน</Table.Head>
								<Table.Head>สถานะ</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#if templates.length === 0}
								<Table.Row>
									<Table.Cell colspan={4} class="h-24 text-center text-muted-foreground">
										ยังไม่มีแบบประเมินนิเทศ
									</Table.Cell>
								</Table.Row>
							{:else}
								{#each templates as template (template.id)}
									<Table.Row>
										<Table.Cell class="font-medium">{template.title}</Table.Cell>
										<Table.Cell>{template.sections.length}</Table.Cell>
										<Table.Cell>{template.ratingMin} - {template.ratingMax}</Table.Cell>
										<Table.Cell><Badge variant="secondary">{template.status}</Badge></Table.Cell>
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
						<Button onclick={loadProgress} disabled={saving}>โหลดรายงาน</Button>
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
												{#if canManageRequests}
													<Button
														size="sm"
														variant="outline"
														onclick={() => submitForReview(observation.id)}
														disabled={saving}
													>
														ส่งตรวจทาน
													</Button>
												{/if}
												{#if canApprove}
													<Button
														size="sm"
														variant="outline"
														onclick={() => approveResult(observation.id)}
														disabled={saving}
													>
														อนุมัติ
													</Button>
													<Button
														size="sm"
														variant="outline"
														onclick={() => publishResult(observation.id)}
														disabled={saving}
													>
														เผยแพร่
													</Button>
													<Button
														size="sm"
														variant="outline"
														onclick={() => returnResult(observation.id)}
														disabled={saving}
													>
														ส่งกลับ
													</Button>
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
			<Button onclick={createCycle} disabled={saving}>
				{#if saving}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
				สร้างรอบนิเทศ
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={createTemplateDialogOpen}>
	<Dialog.Content class="max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>สร้างแบบประเมินพื้นฐาน</Dialog.Title>
			<Dialog.Description>สร้างแบบประเมินเริ่มต้นที่มีหัวข้อคะแนนและข้อเสนอแนะ</Dialog.Description>
		</Dialog.Header>
		<div class="grid gap-4 py-2 lg:grid-cols-2">
			<div class="space-y-2 lg:col-span-2">
				<Label>ชื่อแบบประเมิน</Label>
				<Input bind:value={templateForm.title} placeholder="ชื่อแบบประเมิน" />
			</div>
			<div class="space-y-2 lg:col-span-2">
				<Label>รายละเอียด</Label>
				<Input bind:value={templateForm.description} placeholder="รายละเอียด" />
			</div>
			<div class="space-y-2">
				<Label>คะแนนต่ำสุด</Label>
				<Input type="number" bind:value={templateForm.ratingMin} />
			</div>
			<div class="space-y-2">
				<Label>คะแนนสูงสุด</Label>
				<Input type="number" bind:value={templateForm.ratingMax} />
			</div>
			<div class="space-y-2 lg:col-span-2">
				<Label>หัวข้อแบบคะแนน</Label>
				<Input bind:value={templateForm.ratingLabel} placeholder="หัวข้อแบบคะแนน" />
			</div>
			<div class="space-y-2 lg:col-span-2">
				<Label>หัวข้อแบบข้อความ</Label>
				<Input bind:value={templateForm.textLabel} placeholder="หัวข้อแบบข้อความ" />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (createTemplateDialogOpen = false)}>ยกเลิก</Button>
			<Button onclick={createTemplate} disabled={saving}>
				{#if saving}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
				สร้างแบบประเมิน
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
