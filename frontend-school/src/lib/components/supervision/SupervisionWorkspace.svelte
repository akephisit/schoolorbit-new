<script lang="ts">
	import { onDestroy, tick, untrack } from 'svelte';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import SupervisionCycleDates from '#lib/components/supervision/SupervisionCycleDates.svelte';
	import SupervisionRequestReview from '#lib/components/supervision/SupervisionRequestReview.svelte';
	import { createSupervisionRequestReview } from '#lib/supervision/request-review.svelte.js';
	import SupervisionRequestCard from '#lib/components/supervision/SupervisionRequestCard.svelte';
	import SupervisionBookingCalendar from '#lib/components/supervision/SupervisionBookingCalendar.svelte';
	import { cycleDateRangeError, cycleDateRangeLabel } from '#lib/supervision/booking-range.js';
	import SupervisionCycleDelete from '#lib/components/supervision/SupervisionCycleDelete.svelte';
	import SupervisionLessonPicker from '#lib/components/supervision/SupervisionLessonPicker.svelte';
	import type { SupervisionLessonSelection } from '#lib/supervision/booking-range.js';
	import { savedLessonLabels } from '#lib/supervision/supervision-presentation.js';
	import {
		supervisionStatusLabel as statusLabel,
		currentReturnReason,
		observationWorkflow
	} from '#lib/supervision/supervision-presentation.js';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type {
		ObservationRegion,
		SupervisionWorkspaceRouteData
	} from '#lib/supervision/workspace-route-data.js';
	import { useSupervisionObservationWrites } from '#lib/supervision/observation-writes.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import {
		resolveSupervisionRead,
		SUPERVISION_SCHOOL_REPORT_PERMISSIONS
	} from '#lib/supervision/supervision-access.js';
	import {
		ArrowDown,
		ArrowUp,
		CalendarDays,
		ClipboardCheck,
		Eye,
		FileSignature,
		Inbox,
		Plus,
		Trash2
	} from '@lucide/svelte';
	import { toast } from 'svelte-sonner';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import { academicContextualMenuPath } from '#lib/academic-context/route-context.js';
	import { isAbortError, LatestRequest } from '#lib/async/latest-request.js';
	import {
		acknowledgeSupervisionObservation,
		createSupervisionCycle,
		createSupervisionTemplate,
		getSupervisionTemplate,
		listSupervisionTemplateSummaries,
		type SupervisionTemplateSummary,
		getSupervisionCycleProgress,
		getSupervisionTeacherStatusOverview,
		listSupervisionCycles,
		listSupervisionObservations,
		requestSupervisionObservation,
		submitMySupervisionEvaluation,
		updateSupervisionCycle,
		updateSupervisionTemplate,
		type CreateSupervisionCycleRequest,
		type CreateSupervisionTemplateRequest,
		type SaveEvaluationRequest,
		type SupervisionCycle,
		type SupervisionCycleStatus,
		type SupervisionCycleProgress,
		type SupervisionObservation,
		type SupervisionTeacherStatusRow,
		type SupervisionTemplate,
		type SupervisionTemplateStatus
	} from '#lib/api/supervision.js';
	import {
		calculateRubricDraftSummary,
		createBlankRubricItem,
		createBlankRubricSection,
		createPaperSupervisionRubricSections,
		sectionRubricProgress,
		type RubricFormSection,
		type RubricItemType,
		type RubricResponseDraft
	} from '#lib/utils/supervision-rubric.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can, userPermissions } from '#lib/stores/permissions.js';
	import { cn } from '#lib/utils.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Progress } from '#lib/components/ui/progress/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Skeleton } from '#lib/components/ui/skeleton/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';

	export type SupervisionWorkspaceSection =
		'mine' | 'evaluate' | 'requests' | 'cycles' | 'templates' | 'overview';

	type SectionConfig = {
		title: string;
		description: string;
	};

	type ResponseDraft = RubricResponseDraft;
	type TemplateFormState = {
		title: string;
		description: string;
		status: SupervisionTemplateStatus;
		ratingMin: number;
		ratingMax: number;
		sections: RubricFormSection[];
	};

	type ObservationDetailItem = {
		label: string;
		value: string;
		pending?: boolean;
	};
	type CycleFormState = {
		title: string;
		description: string;
		templateId: string;
		status: SupervisionCycleStatus;
		observationStartDate: string;
		observationEndDate: string;
	};

	const dialogClass = 'flex max-h-[92dvh] w-[calc(100%-2rem)] flex-col gap-0 overflow-hidden p-0';
	const dialogHeaderClass = 'shrink-0 border-b px-4 py-4 pr-12 text-left sm:px-6 sm:pr-14';
	const dialogBodyClass =
		'min-h-0 flex-1 space-y-4 overflow-y-auto overscroll-contain px-4 py-4 sm:px-6';
	const dialogFooterClass = 'shrink-0 border-t flex-row flex-wrap justify-end px-4 py-3 sm:px-6';
	const personalFilters = [
		{ key: 'all', label: 'ทั้งหมด', statuses: null, empty: '' },
		{ key: 'returned', label: 'ต้องแก้ไข', statuses: ['returned'], empty: 'ไม่มีคำขอที่ต้องแก้ไข' },
		{
			key: 'published',
			label: 'รอรับทราบผล',
			statuses: ['published'],
			empty: 'ไม่มีผลนิเทศที่รอรับทราบ'
		},
		{
			key: 'requested',
			label: 'รออนุมัติ',
			statuses: ['requested'],
			empty: 'ไม่มีคำขอที่รออนุมัติ'
		},
		{
			key: 'completed',
			label: 'เสร็จสิ้น',
			statuses: ['completed', 'acknowledged'],
			empty: 'ยังไม่มีรายการที่เสร็จสิ้น'
		}
	] as const;

	const cycleStatusCreateOptions: {
		value: SupervisionCycleStatus;
		label: string;
		description: string;
	}[] = [
		{
			value: 'open',
			label: 'เปิดรับคำขอ',
			description: 'ครูที่มีสิทธิ์จองจะเห็นรอบนี้ในหน้าขอรับนิเทศ'
		},
		{
			value: 'draft',
			label: 'ร่าง',
			description: 'เตรียมข้อมูลไว้ก่อน ยังไม่แสดงในหน้าจองของครู'
		},
		{
			value: 'closed',
			label: 'ปิดรับคำขอ',
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

	const sectionConfigByKey: Record<SupervisionWorkspaceSection, SectionConfig> = {
		mine: {
			title: 'นิเทศของฉัน',
			description: 'จองคาบเรียนจากตารางสอน ติดตามคำขอ และรับทราบผลนิเทศ'
		},
		evaluate: {
			title: 'รายการที่ต้องประเมิน',
			description: 'เปิดแบบประเมินสำหรับรายการนิเทศที่ได้รับมอบหมาย'
		},
		requests: {
			title: 'คำขอจองนิเทศ',
			description: 'ตรวจคำขอจอง กำหนดผู้ประเมิน และดูปฏิทินนัดนิเทศ'
		},
		cycles: {
			title: 'รอบนิเทศ',
			description: 'สร้างและปรับสถานะรอบนิเทศของโรงเรียน'
		},
		templates: {
			title: 'แบบประเมินนิเทศ',
			description: 'สร้าง แก้ไข และดูตัวอย่างแบบประเมินนิเทศการสอน'
		},
		overview: {
			title: 'ภาพรวมนิเทศการสอน',
			description: 'ติดตามสถานะครูและความคืบหน้าของรอบนิเทศ'
		}
	};

	let {
		section,
		routeData
	}: { section: SupervisionWorkspaceSection; routeData: SupervisionWorkspaceRouteData } = $props();
	const academicContext = getAcademicContextStore();
	const observationWrites = useSupervisionObservationWrites();
	const academicYearId = $derived(
		routeData ? routeData.academicYearId : $academicContext.selected.academicYearId
	);
	const academicTermId = $derived(
		routeData ? routeData.academicTermId : $academicContext.selected.academicTermId
	);
	const academicContextOptions = $derived($academicContext.options);
	const calendarAcademicLabel = $derived.by(() => {
		const year = academicContextOptions?.years.find((item) => item.id === academicYearId);
		const term = academicContextOptions?.terms.find(
			(item) => item.id === academicTermId && item.academicYearId === academicYearId
		);
		if (!year) return term?.name ?? '';
		const standardYearLabel = `ปีการศึกษา ${year.year}`;
		const yearName = year.name.trim();
		const yearLabel =
			yearName && yearName !== String(year.year) && yearName !== standardYearLabel
				? `${standardYearLabel} (${yearName})`
				: standardYearLabel;
		if (!academicTermId) return `ตลอด${yearLabel}`;
		return [term?.name, yearLabel].filter(Boolean).join(' ');
	});

	const managementCyclesRequest = new LatestRequest();
	const managementTemplatesRequest = new LatestRequest();
	const templateDetailRequest = new LatestRequest();
	let managementContext = '';
	let observedCycleUrl = '';
	let renderedTeacherCycleId = '';
	let renderedProgressCycleId = '';
	let cyclesLoaded = $state(false);
	let cyclesLoading = $state(true);
	let cyclesError = $state('');
	let templatesLoaded = $state(false);
	let templatesLoading = $state(false);
	let templatesError = $state('');
	let templateSummaries = $state<SupervisionTemplateSummary[]>([]);
	let teacherStatusLoaded = $state(false);
	let teacherStatusError = $state('');
	let templateDetailLoading = $state(false);
	let templateDetailLoaded = $state(false);
	let templateDetailError = $state('');
	let templateDetailMode: 'preview' | 'edit' = 'preview';

	let observationsLoaded = $state(false);
	let observationsLoading = $state(false);
	let observationsReadable = $state(true);
	let observationsError = $state('');
	let queueView = $state('list');
	let bookingCalendarDate = $state('');
	let calendarReviewId = $state('');
	let bookingOpen = $state(false);
	let personalFilter = $state<(typeof personalFilters)[number]['key']>('all');
	let evaluationTemplateLoading = $state(false);
	let evaluationTemplateError = $state('');
	let evaluationFormError = $state('');
	let evaluationValidationAttempted = $state(false);
	let progressError = $state('');
	let saving = $state(false);
	let savingAction = $state<string | null>(null);
	let savingTemplate = $state(false);
	let savingEvaluation = $state.raw<{ observationId: string } | null>(null);
	let evaluationDialogOpen = $state(false);
	let cycles = $state<SupervisionCycle[]>([]);
	let templates = $state<SupervisionTemplate[]>([]);
	let observations = $state<SupervisionObservation[]>([]);
	let selectedCycleId = $state('');
	let evaluationObservationId = $state('');
	let responseDrafts = $state<{ [itemId: string]: ResponseDraft }>({});
	let acknowledgeDialogId = $state('');
	let acknowledgeDrafts = $state<Record<string, { comment: string; error: string }>>({});
	let overviewCycleId = $state('');
	let progress = $state<SupervisionCycleProgress | null>(null);
	let teacherStatusRows = $state<SupervisionTeacherStatusRow[]>([]);
	let loadingProgress = $state(false);
	let loadingTeacherStatus = $state(false);
	let createCycleDialogOpen = $state(false);
	let cycleFormError = $state('');
	let createTemplateDialogOpen = $state(false);
	let templateFormError = $state('');
	let previewTemplateDialogOpen = $state(false);
	let previewTemplateId = $state('');
	let editingTemplateId = $state('');
	let templateWorkflowRevision = 0;
	let cycleForm = $state<CycleFormState>({
		title: '',
		description: '',
		templateId: '',
		status: 'open',
		observationStartDate: '',
		observationEndDate: ''
	});
	let templateForm = $state<TemplateFormState>(createDefaultTemplateForm());
	const observationRequest = new LatestRequest();
	const evaluationTemplateRequest = new LatestRequest();
	const progressRequest = new LatestRequest();
	const teacherStatusRequest = new LatestRequest();

	const templateChoices = $derived(routeData ? templateSummaries : templates);

	const currentUserId = $derived($authStore.user?.id ?? '');
	const routeIdentity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	const sectionConfig = $derived(sectionConfigByKey[section]);
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
	const canReadObservations = $derived(
		$can.hasAny(
			PERMISSIONS.SUPERVISION_READ_OWN,
			PERMISSIONS.SUPERVISION_READ_ASSIGNED,
			PERMISSIONS.SUPERVISION_READ_ORGANIZATION_UNIT,
			PERMISSIONS.SUPERVISION_READ_ORGANIZATION_TREE,
			PERMISSIONS.SUPERVISION_READ_SCHOOL
		) || canManageRequests
	);
	const canReadSchoolReport = $derived($can.hasAny(...SUPERVISION_SCHOOL_REPORT_PERMISSIONS));
	const canReport = $derived(canReadSchoolReport || canManageRequests);
	const activeBookingCycles = $derived(cycles.filter(cycleAcceptsBookings));
	const currentBookingCycle = $derived(
		activeBookingCycles.find((cycle) => cycle.id === selectedCycleId) ??
			activeBookingCycles[0] ??
			null
	);
	const requestedObservations = $derived(
		observations.filter((observation) => observation.status === 'requested')
	);
	const myObservations = $derived(
		observations.filter((observation) => observation.observedUserId === currentUserId)
	);
	const personalResults = $derived.by(() => {
		const ordered = [...myObservations].sort(
			(left, right) => Number(needsPersonalAction(right)) - Number(needsPersonalAction(left))
		);
		return personalFilters.map((filter) => ({
			...filter,
			items: filter.statuses
				? ordered.filter(({ status }) => filter.statuses?.some((value) => value === status))
				: ordered
		}));
	});
	const personalResult = $derived(
		personalResults.find(({ key }) => key === personalFilter) ?? personalResults[0]
	);
	const assignedObservations = $derived(
		observations.filter((observation) =>
			observation.evaluators.some((evaluator) => evaluator.evaluatorUserId === currentUserId)
		)
	);
	const activeAssignedObservations = $derived(
		assignedObservations.filter(canSubmitAssignedEvaluation)
	);
	const submittedAssignedObservations = $derived(
		assignedObservations.filter(hasSubmittedEvaluation)
	);
	const evaluationCalendarObservations = $derived([
		...new Map(
			[...activeAssignedObservations, ...submittedAssignedObservations]
				.filter((observation) => observation.status !== 'requested')
				.map((observation) => [observation.id, observation])
		).values()
	]);
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

	const progressPercent = $derived(
		progress && progress.totalObservations > 0
			? Math.round((progress.completedCount / progress.totalObservations) * 100)
			: 0
	);

	const requestReview = createSupervisionRequestReview({
		getObservation: (id) =>
			section === 'requests' && observationsLoaded && observationsReadable
				? observations.find((item) => item.id === id)
				: undefined,
		hasManagePermission: () => canManageRequests,
		captureOwner: captureMutationOwner,
		beginObservationChange: observationWrites?.beginChange,
		isBusy: () => mutationBusy,
		getSavingAction: () => savingAction,
		setSavingAction: (action) => (savingAction = action),
		onApplied: (updated) => {
			replaceObservation(updated);
			if (calendarReviewId === updated.id && updated.status !== 'requested') calendarReviewId = '';
		},
		onDenied: (id) => {
			if (calendarReviewId === id) calendarReviewId = '';
			void retryObservations();
		},
		allowSaveEvaluators: true
	});
	const calendarReviewObservation = $derived(
		queueView === 'calendar' && observationsLoaded && observationsReadable
			? requestedObservations.find(
					(item) => item.id === calendarReviewId && requestReview.canReview(item)
				)
			: undefined
	);

	let managementRevision = 0;
	let mutationLifetime = 0;
	onDestroy(() => {
		mutationLifetime += 1;
		requestReview.reset();
	});

	function captureMutationOwner() {
		const lifetime = mutationLifetime;
		const revision = managementRevision;
		const context = currentManagementContext();
		const pathname = page.url.pathname;
		const year = page.url.searchParams.get('academicYearId');
		const term = page.url.searchParams.get('academicTermId');
		return () =>
			lifetime === mutationLifetime &&
			revision === managementRevision &&
			context === currentManagementContext() &&
			pathname === page.url.pathname &&
			year === page.url.searchParams.get('academicYearId') &&
			term === page.url.searchParams.get('academicTermId');
	}

	function currentManagementContext() {
		return `${appIdentityKey()}:${section}:${academicYearId ?? ''}:${academicTermId ?? ''}`;
	}

	$effect.pre(() => {
		const source = routeData;
		const identity = routeIdentity;
		const key = `${identity}:${section}:${academicYearId ?? ''}:${academicTermId ?? ''}`;
		if (!source) return;
		untrack(() => {
			if (key !== managementContext) {
				managementContext = key;
				managementRevision += 1;
				requestReview.reset();
				savingAction = null;
				savingTemplate = false;
				cycles = [];
				templates = [];
				templateSummaries = [];
				teacherStatusRows = [];
				observations = [];
				observationsLoaded = false;
				observationsReadable = true;
				bookingCalendarDate = '';
				calendarReviewId = '';
				bookingOpen = false;
				personalFilter = 'all';
				acknowledgeDialogId = '';
				acknowledgeDrafts = {};
				savingEvaluation = null;
				clearEvaluationDraft();
				progress = null;
				progressError = '';
				loadingProgress = false;
				cyclesLoaded = false;
				templatesLoaded = false;
				teacherStatusLoaded = false;
				renderedTeacherCycleId = '';
				renderedProgressCycleId = '';
				createCycleDialogOpen = false;
				createTemplateDialogOpen = false;
				setTemplatePreviewDialogOpen(false);
				resetTemplateForm();
				cycleForm = {
					title: '',
					description: '',
					templateId: '',
					status: 'open',
					observationStartDate: '',
					observationEndDate: ''
				};
				cycleFormError = '';
				overviewCycleId = source.cycleId;
			}
			if (section === 'overview') {
				overviewCycleId = source.cycleId || (cycles[0]?.id ?? '');
				if (renderedProgressCycleId !== overviewCycleId) {
					progress = null;
					renderedProgressCycleId = '';
				}
				if (renderedTeacherCycleId !== overviewCycleId) {
					teacherStatusRows = [];
					teacherStatusLoaded = false;
					renderedTeacherCycleId = '';
				}
			}
			observationsLoading = source.observations !== null;
			observationsError = '';
			observedCycleUrl = source.cycleId;
			cyclesLoading = source.cycles !== null;
			templatesLoading = source.templates !== null;
			loadingProgress = Boolean(source.progress);
			loadingTeacherStatus = source.teacherStatus !== null;
			cyclesError = '';
			templatesError = '';
			teacherStatusError = '';
			progressError = '';
			const observationRead = observationRequest.begin();
			const applyRead = captureObservationRead(
				() =>
					observationRequest.isCurrent(observationRead.revision) &&
					key === currentManagementContext(),
				source.observationsStartedAt
			);
			if (source.observations)
				void Promise.all([source.identity, source.observations]).then(([owner, result]) => {
					if (owner === appIdentityKey()) applyRead(result);
				});
			const cycleRead = managementCyclesRequest.begin();
			const templateRead = managementTemplatesRequest.begin();
			const statusRead = teacherStatusRequest.begin();
			const progressRead = progressRequest.begin();
			if (source.cycles)
				void Promise.all([source.identity, source.cycles]).then(([owner, result]) => {
					if (
						owner !== appIdentityKey() ||
						key !== currentManagementContext() ||
						!managementCyclesRequest.isCurrent(cycleRead.revision)
					)
						return;
					cyclesLoading = false;
					if (!result.ok) {
						cyclesError = result.error;
						return;
					}
					cycles = result.data;
					cyclesLoaded = true;
					selectedCycleId = cycles.some((item) => item.id === selectedCycleId)
						? selectedCycleId
						: (cycles.find((item) => item.status === 'open')?.id ?? cycles[0]?.id ?? '');

					const selected = cycles.some((item) => item.id === source.cycleId)
						? source.cycleId
						: (cycles[0]?.id ?? '');

					overviewCycleId = selected;
					if (section === 'overview' && source.cycleId && selected !== source.cycleId) {
						loadOverviewReports();
					}
				});
			if (source.templates)
				void Promise.all([source.identity, source.templates]).then(([owner, result]) => {
					if (
						owner !== appIdentityKey() ||
						key !== currentManagementContext() ||
						!managementTemplatesRequest.isCurrent(templateRead.revision)
					)
						return;
					templatesLoading = false;
					if (!result.ok) {
						templatesError = result.error;
						return;
					}
					templateSummaries = result.data;
					templatesLoaded = true;
				});
			if (source.progress)
				void Promise.all([source.identity, source.progress]).then(([owner, result]) => {
					if (
						owner !== appIdentityKey() ||
						key !== currentManagementContext() ||
						!progressRequest.isCurrent(progressRead.revision) ||
						!canReadSchoolReport
					)
						return;
					loadingProgress = false;
					if (!result.ok) {
						progressError = result.error;
						return;
					}
					progress = result.data.report;
					renderedProgressCycleId = result.data.cycleId;
				});
			if (source.teacherStatus)
				void Promise.all([source.identity, source.teacherStatus]).then(([owner, result]) => {
					if (
						owner !== appIdentityKey() ||
						key !== currentManagementContext() ||
						!teacherStatusRequest.isCurrent(statusRead.revision)
					)
						return;
					loadingTeacherStatus = false;
					if (!result.ok) {
						teacherStatusError = result.error;
						return;
					}
					teacherStatusRows = result.data.items;
					overviewCycleId = result.data.cycleId;
					renderedTeacherCycleId = result.data.cycleId;
					teacherStatusLoaded = true;
				});
		});
		return () => {
			managementCyclesRequest.abort();
			managementTemplatesRequest.abort();
			teacherStatusRequest.abort();
			templateDetailRequest.abort();
			observationRequest.abort();
			evaluationTemplateRequest.abort();
			progressRequest.abort();
		};
	});

	$effect.pre(() => {
		const selected = page.url.searchParams.get('cycleId') ?? '';
		const managed = routeData;
		if (!managed || section !== 'overview') return;
		untrack(() => {
			if (selected === observedCycleUrl) return;
			observedCycleUrl = selected;
			overviewCycleId = cycles.some((item) => item.id === selected)
				? selected
				: (cycles[0]?.id ?? '');
			loadOverviewReports();
		});
	});

	function selectOverviewCycle(id: string) {
		if (!routeData) {
			overviewCycleId = id;
			return;
		}
		observedCycleUrl = id;
		overviewCycleId = id;
		const url = new URL(page.url.href);

		if (id) url.searchParams.set('cycleId', id);
		else url.searchParams.delete('cycleId');

		goto(resolve(`staff/academic/supervision/overview?${url.searchParams.toString()}`), {
			shallow: true,
			state: page.state
		});
		loadOverviewReports();
	}

	function loadOverviewReports() {
		void loadTeacherStatusOverview();
		void loadProgress();
	}

	async function retryManagementCycles() {
		if (!academicYearId || (section === 'evaluate' && !canReadObservations)) return;

		const key = currentManagementContext();
		const { revision, signal } = managementCyclesRequest.begin();
		cyclesLoading = true;
		cyclesError = '';
		try {
			const items = await listSupervisionCycles(academicYearId, academicTermId, { signal });
			if (!managementCyclesRequest.isCurrent(revision) || key !== currentManagementContext())
				return;
			cycles = items;
			cyclesLoaded = true;
			if (section === 'overview' && !cycles.some((item) => item.id === overviewCycleId)) {
				overviewCycleId = cycles[0]?.id ?? '';
				loadOverviewReports();
			}
		} catch (error) {
			if (!isAbortError(error) && managementCyclesRequest.isCurrent(revision))
				cyclesError = error instanceof Error ? error.message : 'โหลดรอบนิเทศไม่สำเร็จ';
		} finally {
			if (managementCyclesRequest.isCurrent(revision)) cyclesLoading = false;
		}
	}

	async function retryManagementTemplates() {
		if (['mine', 'requests', 'evaluate'].includes(section) && !canReadObservations) return;
		if (section !== 'cycles' && section !== 'templates' && !routeData.templates) return;
		const key = currentManagementContext();
		const { revision, signal } = managementTemplatesRequest.begin();
		templatesLoading = true;
		templatesError = '';
		try {
			const items = await listSupervisionTemplateSummaries({ signal });
			if (!managementTemplatesRequest.isCurrent(revision) || key !== currentManagementContext())
				return;
			templateSummaries = items;
			templatesLoaded = true;
			cycleForm.templateId ||= items[0]?.id ?? '';
		} catch (error) {
			if (!isAbortError(error) && managementTemplatesRequest.isCurrent(revision))
				templatesError = error instanceof Error ? error.message : 'โหลดแบบประเมินไม่สำเร็จ';
		} finally {
			if (managementTemplatesRequest.isCurrent(revision)) templatesLoading = false;
		}
	}

	async function loadManagementTemplateDetail(mode: 'preview' | 'edit', id: string) {
		if (!canManageSchool) return;
		const key = currentManagementContext();
		const { revision, signal } = templateDetailRequest.begin();
		templateDetailMode = mode;
		templateDetailLoading = true;
		templateDetailError = '';
		templateDetailLoaded = mode === 'preview' && templates.some((item) => item.id === id);
		try {
			const item = await getSupervisionTemplate(id, { signal });
			if (
				!templateDetailRequest.isCurrent(revision) ||
				key !== currentManagementContext() ||
				(mode === 'edit'
					? !createTemplateDialogOpen || editingTemplateId !== id
					: !previewTemplateDialogOpen || previewTemplateId !== id)
			)
				return;
			templates = [...templates.filter((value) => value.id !== id), item];
			templateDetailLoaded = true;
			if (mode === 'edit') fillTemplateForm(item);
		} catch (error) {
			if (!isAbortError(error) && templateDetailRequest.isCurrent(revision))
				templateDetailError =
					error instanceof Error ? error.message : 'โหลดรายละเอียดแบบประเมินไม่สำเร็จ';
		} finally {
			if (templateDetailRequest.isCurrent(revision)) templateDetailLoading = false;
		}
	}

	function formatDate(value?: string | null): string {
		if (!value) return '-';
		return new Intl.DateTimeFormat('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short'
		}).format(new Date(value));
	}

	function academicYearLabel(yearId: string): string {
		return (
			academicContextOptions?.years.find((year) => year.id === yearId)?.name ?? 'ไม่พบปีการศึกษา'
		);
	}

	function academicTermLabel(termId?: string | null): string {
		if (!termId) return 'ทั้งปี';

		return (
			academicContextOptions?.terms.find((term) => term.id === termId)?.name ?? 'ไม่พบภาคเรียน'
		);
	}

	function cycleContextLabel(cycle: SupervisionCycle): string {
		return `${academicYearLabel(cycle.academicYearId)} · ${academicTermLabel(cycle.academicTermId)}`;
	}

	function openCreateCycleDialog() {
		if (!canManageSchool) return;
		if (!academicYearId) {
			toast.error('กรุณาเลือกปีการศึกษาก่อน');
			return;
		}
		cycleFormError = '';
		createCycleDialogOpen = true;
		if (routeData) void retryManagementTemplates();
	}

	function cycleLabel(cycle: SupervisionCycle): string {
		return `${cycle.title} - ${cycleContextLabel(cycle)}`;
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

	function openTemplatePreviewDialog(template: SupervisionTemplate | SupervisionTemplateSummary) {
		previewTemplateId = template.id;
		previewTemplateDialogOpen = true;
		if (routeData) void loadManagementTemplateDetail('preview', template.id);
	}

	function setTemplatePreviewDialogOpen(open: boolean) {
		previewTemplateDialogOpen = open;
		if (!open) {
			previewTemplateId = '';
			if (templateDetailMode === 'preview') {
				templateDetailRequest.abort();
				templateDetailLoading = false;
			}
		}
	}

	function cycleAcceptsBookings(cycle: SupervisionCycle): boolean {
		return cycle.status === 'open';
	}

	function observationDetailHref(item: SupervisionObservation): string {
		return academicContextualMenuPath(
			`/staff/academic/supervision/${item.id}`,
			{ academicYearId: item.academicYearId, academicTermId: item.academicTermId ?? null },
			academicContextOptions
		);
	}

	function observationSubjectLabel(observation: SupervisionObservation): string {
		return savedLessonLabels(observation).subject;
	}
	function observationClassroomLabel(observation: SupervisionObservation): string {
		return savedLessonLabels(observation).homeroom;
	}
	function observationPeriodLabel(observation: SupervisionObservation): string {
		return savedLessonLabels(observation).period;
	}
	function observationRoomLabel(observation: SupervisionObservation): string {
		return savedLessonLabels(observation).room;
	}

	function observationEvaluatorNames(observation: SupervisionObservation): string {
		if (observation.evaluators.length === 0) return 'ยังไม่มอบหมาย';
		return observation.evaluators
			.map((evaluator) => evaluator.evaluatorDisplayName ?? 'ผู้ประเมิน')
			.join(', ');
	}

	function canCorrectPersonalRequest(observation: SupervisionObservation): boolean {
		return (
			observation.status === 'returned' &&
			canRequest &&
			!observation.evaluators.some(({ status }) => status === 'submitted')
		);
	}

	function needsPersonalAction(observation: SupervisionObservation): boolean {
		return observation.status === 'published' || canCorrectPersonalRequest(observation);
	}

	function selectPersonalFilter(value: string) {
		const filter = personalFilters.find(({ key }) => key === value);
		if (filter) personalFilter = filter.key;
	}

	function observationLessonFacts(observation: SupervisionObservation): ObservationDetailItem[] {
		return [
			{ label: 'วันที่/เวลา', value: formatDate(observation.observedAt) },
			{ label: 'วิชา', value: observationSubjectLabel(observation) },
			{ label: 'คาบ', value: observationPeriodLabel(observation) }
		];
	}

	function observationAdditionalFacts(
		observation: SupervisionObservation
	): ObservationDetailItem[] {
		return [
			{ label: 'ห้อง/ชั้นเรียน', value: observationClassroomLabel(observation) },
			...observationReferenceFacts(observation),
			{ label: 'ผู้นิเทศ', value: observationEvaluatorNames(observation) }
		];
	}

	function observationReferenceFacts(observation: SupervisionObservation): ObservationDetailItem[] {
		return [
			{ label: 'ห้องเรียน', value: observationRoomLabel(observation) },
			{
				label: 'รอบนิเทศ',
				value: requestCycleLabel(observation),
				pending: cyclesLoading && !cyclesLoaded
			},
			{
				label: 'แบบประเมิน',
				value: requestTemplateTitle(observation),
				pending: templatesLoading && !templatesLoaded
			},
			{ label: 'ส่งคำขอ', value: formatDate(observation.requestedAt) },
			{ label: 'อนุมัติ', value: formatDate(observation.approvedAt) }
		];
	}

	function observationDetailGrid(observation: SupervisionObservation): ObservationDetailItem[] {
		return [...observationLessonFacts(observation), ...observationAdditionalFacts(observation)];
	}

	function shouldLoadObservations() {
		return routeData.observations !== null;
	}
	function captureObservationRead(ownsRead: () => boolean, startedAt?: number) {
		const read =
			section === 'mine' && academicYearId && academicTermId
				? observationWrites?.guardRead({ academicYearId, academicTermId }, startedAt)
				: undefined;
		return (result: RouteLoadResult<ObservationRegion>) => {
			if (!ownsRead()) return;
			const denied = result.ok && !result.data.readable;
			if (!denied && read && !read.isCurrent()) {
				void read.whenSettled().then(() => {
					if (ownsRead()) void retryObservations();
				});
				return;
			}
			observationsLoading = false;
			if (!result.ok) {
				observationsError = result.error;
				return;
			}
			applyObservations(result.data.items, result.data.readable);
		};
	}

	function applyObservations(items: SupervisionObservation[], readable: boolean, freshRead = true) {
		observations = items;
		observationsReadable = readable;
		observationsLoaded ||= freshRead;
		if (section === 'evaluate') {
			if (!readable) {
				managementRevision += 1;
				bookingCalendarDate = '';
				savingEvaluation = null;
				clearEvaluationDraft();
			} else if (freshRead) {
				if (savingEvaluation && !hasActionableAssignment(savingEvaluation.observationId)) {
					savingEvaluation = null;
				}
				if (evaluationObservationId && !hasActionableAssignment(evaluationObservationId)) {
					clearEvaluationDraft();
				}
			}
		}
		requestReview.reconcile(freshRead);
		if (calendarReviewId && !calendarReviewObservation) calendarReviewId = '';
	}
	async function retryObservations() {
		if (!academicYearId || !canReadObservations) return;
		const { revision, signal } = observationRequest.begin();
		const owner = currentManagementContext();
		const applyRead = captureObservationRead(
			() => observationRequest.isCurrent(revision) && owner === currentManagementContext()
		);
		observationsLoading = true;
		observationsError = '';
		const read = resolveSupervisionRead(
			listSupervisionObservations(
				{ academicYearId, ...(academicTermId ? { academicTermId } : {}) },
				{ signal }
			)
		).then((items) => ({ items: items ?? [], readable: items !== null }));
		applyRead(await captureRouteLoad(read, 'โหลดรายการนิเทศไม่สำเร็จ'));
	}
	function replaceCycle(cycle: SupervisionCycle) {
		if (routeData) {
			managementCyclesRequest.abort();
			cyclesLoading = false;
			cyclesLoaded = true;
		}
		cycles = cycles.some((item) => item.id === cycle.id)
			? cycles.map((item) => (item.id === cycle.id ? cycle : item))
			: [cycle, ...cycles];
		selectedCycleId ||= cycle.id;
		overviewCycleId ||= cycle.id;
	}

	function replaceTemplate(template: SupervisionTemplate) {
		if (routeData) {
			managementTemplatesRequest.abort();
			templatesLoading = false;
			templatesLoaded = true;
			const summary: SupervisionTemplateSummary = {
				id: template.id,
				title: template.title,
				status: template.status,
				ratingMin: template.ratingMin,
				ratingMax: template.ratingMax,
				sectionCount: template.sections.length,
				itemCount: templateItemCount(template)
			};
			templateSummaries = templateSummaries.some((item) => item.id === template.id)
				? templateSummaries.map((item) => (item.id === template.id ? summary : item))
				: [summary, ...templateSummaries];
		}
		templates = templates.some((item) => item.id === template.id)
			? templates.map((item) => (item.id === template.id ? template : item))
			: [template, ...templates];
		cycleForm.templateId ||= template.id;
	}

	function replaceObservation(observation: SupervisionObservation) {
		observationRequest.abort();
		observationsLoading = false;
		observationsError = '';
		applyObservations(
			observations.some((item) => item.id === observation.id)
				? observations.map((item) => (item.id === observation.id ? observation : item))
				: [observation, ...observations],
			observationsReadable,
			false
		);
	}

	function currentUserEvaluator(observation: SupervisionObservation) {
		return observation.evaluators.find((evaluator) => evaluator.evaluatorUserId === currentUserId);
	}

	function hasSubmittedEvaluation(observation: SupervisionObservation): boolean {
		return currentUserEvaluator(observation)?.status === 'submitted';
	}

	function hasActionableAssignment(observationId: string) {
		return observations.some(
			(observation) => observation.id === observationId && canSubmitAssignedEvaluation(observation)
		);
	}

	function canSubmitAssignedEvaluation(observation: SupervisionObservation): boolean {
		const evaluator = currentUserEvaluator(observation);
		return Boolean(
			evaluator &&
			evaluator.status !== 'submitted' &&
			(observation.status === 'planned' || observation.status === 'in_progress')
		);
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

	async function createBookingRequest(selection: SupervisionLessonSelection) {
		if (!canRequest || !currentBookingCycle || !academicTermId)
			throw new Error('รอบนิเทศยังไม่เปิดรับคำขอ');
		const ownsMutation = captureMutationOwner();
		const finishChange = observationWrites?.beginChange({
			academicYearId: currentBookingCycle.academicYearId,
			academicTermId
		});
		let incomplete: boolean;
		try {
			const response = await requestSupervisionObservation({
				cycleId: currentBookingCycle.id,
				academicTermId,
				...selection
			});
			if (!ownsMutation() || !canRequest) return;
			const booking = requireMutationData(response, 'ส่งคำขอไม่สำเร็จ');
			if (!canReadObservations) return;
			incomplete = !observationsLoaded;
			replaceObservation(booking);
		} finally {
			finishChange?.();
		}
		if (incomplete && ownsMutation()) void retryObservations();
	}

	function requestCycleLabel(observation: SupervisionObservation): string {
		const cycle = cycles.find((item) => item.id === observation.cycleId);
		return cycle ? cycleLabel(cycle) : cyclesLoaded ? 'ไม่พบรอบนิเทศ' : '—';
	}

	function requestTemplateTitle(observation: SupervisionObservation): string {
		return (
			templateSummaries.find((template) => template.id === observation.templateId)?.title ??
			(templatesLoaded ? 'ไม่พบแบบประเมิน' : '—')
		);
	}

	function prepareEvaluationDraft(observation: SupervisionObservation) {
		if (!canEvaluate || !canReadObservations || !observationsReadable) return;
		if (!canSubmitAssignedEvaluation(observation)) return;
		evaluationObservationId = observation.id;
		responseDrafts = {};
		evaluationFormError = '';
		evaluationValidationAttempted = false;
		evaluationDialogOpen = true;
		void loadEvaluationTemplate();
	}
	async function loadEvaluationTemplate() {
		if (!canEvaluate || !evaluationDialogOpen || !selectedEvaluation) return;
		const observationId = selectedEvaluation.id;
		const templateId = selectedEvaluation.templateId;
		const { revision, signal } = evaluationTemplateRequest.begin();
		evaluationTemplateLoading = true;
		evaluationTemplateError = '';
		try {
			const item = await getSupervisionTemplate(templateId, { signal });
			if (
				!evaluationTemplateRequest.isCurrent(revision) ||
				!evaluationDialogOpen ||
				evaluationObservationId !== observationId
			)
				return;
			templates = [...templates.filter((value) => value.id !== templateId), item];
			const drafts: Record<string, ResponseDraft> = {};
			for (const section of item.sections)
				for (const field of section.items) drafts[field.id] = { ratingScore: '', textResponse: '' };
			responseDrafts = drafts;
		} catch (error) {
			if (!isAbortError(error) && evaluationTemplateRequest.isCurrent(revision))
				evaluationTemplateError =
					error instanceof Error ? error.message : 'โหลดแบบประเมินไม่สำเร็จ';
		} finally {
			if (evaluationTemplateRequest.isCurrent(revision)) evaluationTemplateLoading = false;
		}
	}
	function clearEvaluationDraft() {
		evaluationTemplateRequest.abort();
		evaluationTemplateLoading = false;
		evaluationTemplateError = '';
		evaluationFormError = '';
		evaluationValidationAttempted = false;
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

	const missingRequiredEvaluationItems = $derived.by(() => {
		const missing: RubricFormSection['items'] = [];
		for (const section of selectedEvaluationRubricSections) {
			for (const item of section.items) {
				if (!item.required) continue;
				const draft = responseDrafts[item.localId] ?? { ratingScore: '', textResponse: '' };
				const answered =
					item.itemType === 'rating'
						? Boolean(draft.ratingScore)
						: Boolean(draft.textResponse.trim());
				if (!answered) missing.push(item);
			}
		}
		return missing;
	});

	async function saveEvaluation() {
		if (
			!canEvaluate ||
			!canReadObservations ||
			!observationsReadable ||
			savingEvaluation !== null ||
			!selectedEvaluation ||
			!canSubmitAssignedEvaluation(selectedEvaluation) ||
			evaluationTemplateLoading ||
			evaluationTemplateError ||
			!selectedEvaluationTemplate
		)
			return;
		const observationId = evaluationObservationId;
		if (!observationId) return;
		evaluationFormError = '';
		const firstMissing = missingRequiredEvaluationItems[0];
		if (firstMissing) {
			const ownsFocus = captureMutationOwner();
			evaluationValidationAttempted = true;
			await tick();
			if (!ownsFocus() || !evaluationDialogOpen || evaluationObservationId !== observationId)
				return;
			const answer = document.getElementById(`evaluation-answer-${firstMissing.localId}`);
			const target = answer ?? document.getElementById(`evaluation-field-${firstMissing.localId}`);
			target?.scrollIntoView({ block: 'nearest' });
			answer?.focus({ preventScroll: true });
			return;
		}

		const submission = { observationId };
		savingEvaluation = submission;
		const ownsMutation = captureMutationOwner();
		const isCurrentSubmission = () => savingEvaluation === submission && ownsMutation();
		const finishChange = observationWrites?.beginChange(selectedEvaluation);
		try {
			const payload = evaluationPayload();
			const response = await submitMySupervisionEvaluation(observationId, payload);
			if (!isCurrentSubmission()) return;
			const observation = requireMutationData(response, 'บันทึกผลประเมินไม่สำเร็จ');
			const refreshPending = observationsLoading;
			replaceObservation(observation);
			if (evaluationObservationId === observationId) clearEvaluationDraft();
			toast.success('ส่งผลประเมินแล้ว');
			if (refreshPending) void retryObservations();
		} catch (error) {
			if (!isCurrentSubmission()) return;
			const message = error instanceof Error ? error.message : 'บันทึกผลประเมินไม่สำเร็จ';
			if (evaluationDialogOpen && evaluationObservationId === observationId)
				evaluationFormError = message;
			else toast.error(message);
		} finally {
			finishChange?.();
			if (isCurrentSubmission()) savingEvaluation = null;
		}
	}

	function setAcknowledgeDialogOpen(id: string, open: boolean) {
		if (open) {
			acknowledgeDrafts[id] ??= { comment: '', error: '' };
			acknowledgeDialogId = id;
		} else if (acknowledgeDialogId === id && savingAction !== `acknowledge-result:${id}`) {
			acknowledgeDialogId = '';
		}
	}

	async function acknowledgeResult(id: string) {
		const draft = acknowledgeDrafts[id];
		const current = observations.find((observation) => observation.id === id);
		if (!draft || !current || mutationBusy || acknowledgeDialogId !== id) return;
		draft.error = '';
		savingAction = `acknowledge-result:${id}`;
		const ownsMutation = captureMutationOwner();
		const finishChange = observationWrites?.beginChange(current);
		try {
			const response = await acknowledgeSupervisionObservation(id, {
				comment: draft.comment || null
			});
			if (!ownsMutation()) return;
			const observation = requireMutationData(response, 'รับทราบผลไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('รับทราบผลนิเทศแล้ว');
			acknowledgeDialogId = '';
			delete acknowledgeDrafts[id];
			await tick();
			if (ownsMutation())
				document.getElementById(`supervision-personal-filter-${personalFilter}`)?.focus();
		} catch (error) {
			if (!ownsMutation()) return;
			draft.error = error instanceof Error ? error.message : 'รับทราบผลไม่สำเร็จ';
		} finally {
			finishChange?.();
			if (ownsMutation()) savingAction = null;
		}
	}

	async function createCycle() {
		if (!canManageSchool || mutationBusy) return;
		cycleFormError = '';
		if (!academicYearId) {
			cycleFormError = 'กรุณาเลือกปีการศึกษาก่อน';
			return;
		}
		if (!cycleForm.title.trim() || !cycleForm.templateId) {
			cycleFormError = 'กรอกชื่อรอบและเลือกแบบประเมินให้ครบ';
			return;
		}

		const dateError = cycleDateRangeError(
			cycleForm.observationStartDate,
			cycleForm.observationEndDate
		);
		if (dateError) {
			cycleFormError = dateError;
			return;
		}

		const payload: CreateSupervisionCycleRequest = {
			observationStartDate: cycleForm.observationStartDate,
			observationEndDate: cycleForm.observationEndDate,
			academicYearId,
			academicTermId,
			title: cycleForm.title,
			description: cycleForm.description || null,
			templateId: cycleForm.templateId,
			status: cycleForm.status,
			targets: [{ targetType: 'school', requiredObservations: 1, priority: 100 }]
		};

		savingAction = 'create-cycle';
		const ownsMutation = captureMutationOwner();
		try {
			const response = await createSupervisionCycle(payload);
			if (!ownsMutation()) return;
			const cycle = requireMutationData(response, 'สร้างรอบนิเทศไม่สำเร็จ');
			replaceCycle(cycle);
			toast.success('สร้างรอบนิเทศแล้ว');
			cycleForm.title = '';
			cycleForm.description = '';
			cycleForm.observationStartDate = '';
			cycleForm.observationEndDate = '';
			createCycleDialogOpen = false;
		} catch (error) {
			if (!ownsMutation()) return;
			cycleFormError = error instanceof Error ? error.message : 'สร้างรอบนิเทศไม่สำเร็จ';
		} finally {
			if (ownsMutation()) savingAction = null;
		}
	}

	async function saveCycleDates(
		cycleId: string,
		dates: Pick<CreateSupervisionCycleRequest, 'observationStartDate' | 'observationEndDate'>
	) {
		if (!canManageSchool || mutationBusy) throw new Error('ยังบันทึกช่วงวันที่ไม่ได้');
		const ownsMutation = captureMutationOwner();
		savingAction = `cycle-dates:${cycleId}`;
		try {
			const response = await updateSupervisionCycle(cycleId, dates);
			if (!ownsMutation()) return;
			replaceCycle(requireMutationData(response, 'บันทึกช่วงวันที่ไม่สำเร็จ'));
		} catch (failure) {
			if (ownsMutation()) throw failure;
		} finally {
			if (ownsMutation()) savingAction = null;
		}
	}

	async function setCycleStatus(cycle: SupervisionCycle, status: SupervisionCycleStatus) {
		if (!canManageSchool) return;
		if (cycle.status === status) return;

		savingAction = `cycle-status:${cycle.id}:${status}`;
		const ownsMutation = captureMutationOwner();
		try {
			const response = await updateSupervisionCycle(cycle.id, { status });
			if (!ownsMutation()) return;
			const updatedCycle = requireMutationData(response, 'เปลี่ยนสถานะรอบนิเทศไม่สำเร็จ');
			replaceCycle(updatedCycle);
			toast.success(`เปลี่ยนสถานะรอบนิเทศเป็น${statusLabel(status)}แล้ว`);
		} catch (error) {
			if (!ownsMutation()) return;
			toast.error(error instanceof Error ? error.message : 'เปลี่ยนสถานะรอบนิเทศไม่สำเร็จ');
		} finally {
			if (ownsMutation()) savingAction = null;
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
		templateFormError = '';
		templateForm = createDefaultTemplateForm();
	}

	function openCreateTemplateDialog() {
		if (!canManageSchool) return;
		templateWorkflowRevision += 1;
		templateDetailRequest.abort();
		templateDetailLoading = false;
		templateDetailLoaded = true;
		templateDetailError = '';
		resetTemplateForm();
		createTemplateDialogOpen = true;
	}

	function openEditTemplateDialog(template: SupervisionTemplate | SupervisionTemplateSummary) {
		if (!canManageSchool) return;
		templateWorkflowRevision += 1;
		resetTemplateForm();
		editingTemplateId = template.id;
		createTemplateDialogOpen = true;
		if (routeData) {
			void loadManagementTemplateDetail('edit', template.id);
			return;
		}
		if ('sections' in template) fillTemplateForm(template);
	}

	function fillTemplateForm(template: SupervisionTemplate) {
		editingTemplateId = template.id;
		templateForm = {
			title: template.title,
			description: template.description ?? '',
			status: template.status,
			ratingMin: template.ratingMin,
			ratingMax: template.ratingMax,
			sections: templateSectionsToRubricForm(template)
		};
	}

	function setTemplateEditorDialogOpen(open: boolean) {
		createTemplateDialogOpen = open;
		if (!open && templateDetailMode === 'edit') {
			templateDetailRequest.abort();
			templateDetailLoading = false;
		}
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
			}))
		};
	}

	function validateTemplateForm(): boolean {
		if (!templateForm.title.trim()) {
			templateFormError = 'กรอกชื่อแบบประเมิน';
			return false;
		}
		if (Number(templateForm.ratingMin) >= Number(templateForm.ratingMax)) {
			templateFormError = 'คะแนนต่ำสุดต้องน้อยกว่าคะแนนสูงสุด';
			return false;
		}
		if (templateForm.sections.length === 0) {
			templateFormError = 'เพิ่มหมวดแบบประเมินอย่างน้อย 1 หมวด';
			return false;
		}
		if (templateForm.sections.every((section) => section.items.length === 0)) {
			templateFormError = 'เพิ่มหัวข้อประเมินอย่างน้อย 1 ข้อ';
			return false;
		}
		if (templateForm.sections.some((section) => !section.title.trim())) {
			templateFormError = 'กรอกชื่อหมวดให้ครบ';
			return false;
		}
		if (templateForm.sections.some((section) => section.items.some((item) => !item.label.trim()))) {
			templateFormError = 'กรอกหัวข้อประเมินให้ครบ';
			return false;
		}
		return true;
	}

	async function createTemplate() {
		if (
			!canManageSchool ||
			savingTemplate ||
			!templateDetailLoaded ||
			templateDetailLoading ||
			templateDetailError
		)
			return;
		templateFormError = '';
		if (!validateTemplateForm()) return;

		savingTemplate = true;
		const ownsMutation = captureMutationOwner();
		const workflowRevision = templateWorkflowRevision;
		const submittedTemplateId = editingTemplateId;
		try {
			const payload = templatePayload();
			const response = submittedTemplateId
				? await updateSupervisionTemplate(submittedTemplateId, payload)
				: await createSupervisionTemplate({
						...payload,
						steps: [
							{
								stepOrder: 1,
								stepCode: 'acknowledge',
								label: 'ครูรับทราบผล',
								actorKind: 'observed_teacher',
								actionKind: 'acknowledge',
								required: true
							}
						]
					});
			if (!ownsMutation()) return;
			if (!response.success) throw new Error(response.error || 'บันทึกแบบประเมินไม่สำเร็จ');
			if (response.data) {
				replaceTemplate(response.data);
			} else {
				await retryManagementTemplates();
			}
			if (!ownsMutation()) return;
			toast.success(submittedTemplateId ? 'แก้ไขแบบประเมินนิเทศแล้ว' : 'สร้างแบบประเมินนิเทศแล้ว');
			if (workflowRevision === templateWorkflowRevision) {
				resetTemplateForm();
				createTemplateDialogOpen = false;
			}
		} catch (error) {
			if (!ownsMutation()) return;
			const message = error instanceof Error ? error.message : 'บันทึกแบบประเมินไม่สำเร็จ';
			if (createTemplateDialogOpen && workflowRevision === templateWorkflowRevision)
				templateFormError = message;
			else toast.error(message);
		} finally {
			if (ownsMutation()) savingTemplate = false;
		}
	}

	async function loadProgress() {
		if (!canReadSchoolReport) return;
		const cycleId = overviewCycleId;
		if (!cycleId && !cyclesLoaded) {
			await retryManagementCycles();
			return;
		}
		const key = currentManagementContext();
		const { revision, signal } = progressRequest.begin();
		if (renderedProgressCycleId !== cycleId) progress = null;
		progressError = '';
		loadingProgress = Boolean(cycleId);
		if (!cycleId) return;
		try {
			const report = await getSupervisionCycleProgress(cycleId, { signal });
			if (
				!progressRequest.isCurrent(revision) ||
				cycleId !== overviewCycleId ||
				key !== currentManagementContext()
			)
				return;
			progress = report;
			renderedProgressCycleId = cycleId;
		} catch (error) {
			if (isAbortError(error) || !progressRequest.isCurrent(revision)) return;
			progressError = error instanceof Error ? error.message : 'โหลดความคืบหน้ารอบนิเทศไม่สำเร็จ';
		} finally {
			if (progressRequest.isCurrent(revision)) loadingProgress = false;
		}
	}

	async function loadTeacherStatusOverview() {
		if (!canReport) return;
		const cycleId = overviewCycleId;
		if (routeData && !cycleId && !cyclesLoaded) {
			await retryManagementCycles();
			return;
		}
		const key = currentManagementContext();
		const { revision, signal } = teacherStatusRequest.begin();
		if (routeData && renderedTeacherCycleId !== cycleId) {
			teacherStatusRows = [];
			teacherStatusLoaded = false;
		}
		teacherStatusError = '';
		if (!cycleId) {
			teacherStatusRows = [];
			teacherStatusLoaded = true;
			loadingTeacherStatus = false;
			renderedTeacherCycleId = '';
			return;
		}
		loadingTeacherStatus = true;
		try {
			const rows = await getSupervisionTeacherStatusOverview(cycleId, { signal });
			if (
				!teacherStatusRequest.isCurrent(revision) ||
				cycleId !== overviewCycleId ||
				key !== currentManagementContext()
			)
				return;
			teacherStatusRows = rows;
			teacherStatusLoaded = true;
			renderedTeacherCycleId = cycleId;
		} catch (error) {
			if (isAbortError(error) || !teacherStatusRequest.isCurrent(revision)) return;
			if (routeData)
				teacherStatusError = error instanceof Error ? error.message : 'โหลดสถานะครูไม่สำเร็จ';
			else toast.error(error instanceof Error ? error.message : 'โหลดภาพรวมสถานะครูไม่สำเร็จ');
		} finally {
			if (teacherStatusRequest.isCurrent(revision)) loadingTeacherStatus = false;
		}
	}
</script>

{#snippet managementFeedback(loaded: boolean, pending: boolean, error: string, retry: () => void)}
	{#if pending && !loaded}
		<div role="status" aria-label="กำลังโหลดข้อมูล"><PageSkeleton variant="table" /></div>
	{/if}
	{#if error}
		<PageState
			variant="error"
			title="โหลดข้อมูลไม่สำเร็จ"
			description={error}
			actionLabel="ลองใหม่"
			onaction={retry}
		/>
	{/if}
{/snippet}

{#snippet observationFeedback()}
	{#if observationsLoading && !observationsLoaded && (section !== 'mine' || myObservations.length === 0)}
		<div
			role="status"
			aria-label="กำลังโหลดรายการนิเทศ"
			class="space-y-3 rounded-xl border bg-card p-4 sm:p-5"
		>
			<Skeleton class="h-5 w-40 max-w-full" />
			<Skeleton class="h-4 w-64 max-w-full" />
		</div>
	{/if}
	{@render managementFeedback(true, false, observationsError, retryObservations)}
{/snippet}

{#snippet referenceErrors(includeTemplates: boolean)}
	{#if cyclesError || (includeTemplates && templatesError)}
		<div class="space-y-3" data-testid="supervision-references">
			{#if cyclesError}
				<PageState
					variant="error"
					title="โหลดรอบนิเทศไม่สำเร็จ"
					description={cyclesError}
					actionLabel="ลองโหลดรอบนิเทศอีกครั้ง"
					onaction={retryManagementCycles}
				/>
			{/if}
			{#if includeTemplates && templatesError}
				<PageState
					variant="error"
					title="โหลดข้อมูลแบบประเมินไม่สำเร็จ"
					description={templatesError}
					actionLabel="ลองโหลดแบบประเมินอีกครั้ง"
					onaction={retryManagementTemplates}
				/>
			{/if}
		</div>
	{/if}
{/snippet}

{#snippet summaryMetrics(
	metrics: { label: string; value: number | string | null }[],
	label: string,
	testId?: string
)}
	<dl
		class="grid grid-cols-2 gap-px overflow-hidden rounded-xl border bg-border sm:grid-cols-4"
		aria-label={label}
		data-testid={testId}
	>
		{#each metrics as metric (metric.label)}
			<div class="min-w-0 space-y-1 bg-card px-3 py-3 sm:px-4">
				<dt class="text-sm text-muted-foreground">{metric.label}</dt>
				<dd class="flex min-h-8 items-center text-2xl font-semibold tabular-nums">
					{#if metric.value === null}<Skeleton class="h-5 w-9" />{:else}{metric.value}{/if}
				</dd>
			</div>
		{/each}
	</dl>
{/snippet}

{#snippet sectionHeading(title: string, count: number | null, id?: string, unit = 'รายการ')}
	<div class="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
		<h2 {id} class="text-lg font-semibold">{title}</h2>
		{#if count !== null}<p class="text-sm text-muted-foreground tabular-nums">
				{count}
				{unit}
			</p>{/if}
	</div>
{/snippet}

{#snippet observationFactItems(items: ObservationDetailItem[])}
	<dl
		class="grid gap-x-4 gap-y-3 border-y py-3 text-sm sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5"
	>
		{#each items as detail (detail.label)}
			<div class="min-w-0 space-y-0.5">
				<dt class="text-xs text-muted-foreground">{detail.label}</dt>
				<dd class="wrap-anywhere font-medium">
					{#if detail.pending}
						<Skeleton class="h-4 w-24 max-w-full" />
						<span class="sr-only">กำลังโหลด{detail.label}</span>
					{:else}{detail.value}{/if}
				</dd>
			</div>
		{/each}
	</dl>
{/snippet}

{#snippet observationFacts(observation: SupervisionObservation, secondaryOnly = false)}
	{@render observationFactItems(
		secondaryOnly
			? [
					{ label: 'นิเทศใคร', value: observation.observedDisplayName ?? '—' },
					...observationAdditionalFacts(observation)
				]
			: observationDetailGrid(observation)
	)}
{/snippet}

{#snippet headerActions()}
	{#if section === 'mine' && canRequest}
		<Button
			id="supervision-booking-toggle"
			variant={bookingOpen ? 'outline' : 'default'}
			aria-expanded={bookingOpen}
			aria-controls={bookingOpen ? 'supervision-booking-panel' : undefined}
			onclick={() => (bookingOpen = !bookingOpen)}
		>
			{#if bookingOpen}ปิดหน้าจอง{:else}<Plus class="size-4" aria-hidden="true" />จองคาบนิเทศ{/if}
		</Button>
	{:else if section === 'cycles'}
		<Button onclick={openCreateCycleDialog}>
			<Plus class="size-4" aria-hidden="true" />สร้างรอบนิเทศ
		</Button>
	{:else if section === 'templates'}
		<Button onclick={openCreateTemplateDialog}>
			<Plus class="size-4" aria-hidden="true" />สร้างแบบประเมิน
		</Button>
	{/if}
{/snippet}

<PageShell
	title={sectionConfig.title}
	description={sectionConfig.description}
	class="pt-4 md:pt-(--school-page-padding)"
	contentClass="flex flex-col gap-5 [&>*]:mt-0"
	actions={(section === 'mine' && canRequest) ||
	((section === 'cycles' || section === 'templates') && canManageSchool)
		? headerActions
		: undefined}
>
	{#if section === 'mine'}
		<div class="space-y-5">
			{#if bookingOpen && canRequest}
				<Card.Root
					id="supervision-booking-panel"
					class="gap-0 overflow-hidden border-primary/20 py-0"
				>
					<Card.Header
						class="flex flex-wrap items-start justify-between gap-3 border-b bg-primary/5 px-4 py-4 sm:px-6"
					>
						<div class="space-y-1">
							<h2 class="text-lg font-semibold">จองคาบนิเทศของฉัน</h2>
							<Card.Description
								>เลือกหนึ่งคาบหรือหลายคาบต่อเนื่องของห้องเดียวกันในคำขอเดียว</Card.Description
							>
						</div>
					</Card.Header>
					<Card.Content class="space-y-4 px-4 py-4 sm:px-6">
						{#if cyclesLoading && !cyclesLoaded}
							<div
								role="status"
								aria-label="กำลังโหลดรอบนิเทศ"
								class="space-y-2 rounded-lg border bg-muted/25 p-4"
							>
								<Skeleton class="h-4 w-24" />
								<Skeleton class="h-9 w-full max-w-sm" />
							</div>
						{/if}
						{@render referenceErrors(false)}
						{#if cyclesLoaded && currentBookingCycle && academicYearId && academicTermId}
							<div
								class="grid gap-4 rounded-lg border bg-muted/25 p-4 md:grid-cols-2 md:items-start"
							>
								<div class="min-w-0 space-y-2">
									<Label for="booking-cycle">รอบนิเทศ</Label><Select.Root
										type="single"
										value={currentBookingCycle.id}
										onValueChange={(value) => (selectedCycleId = value)}
										><Select.Trigger id="booking-cycle" class="w-full" aria-label="เลือกรอบนิเทศ"
											>{currentBookingCycle.title}</Select.Trigger
										><Select.Content
											>{#each activeBookingCycles as cycle (cycle.id)}<Select.Item value={cycle.id}
													>{cycle.title}</Select.Item
												>{/each}</Select.Content
										></Select.Root
									>
								</div>
								<div class="min-w-0 space-y-1" data-testid="supervision-cycle-dates">
									<p class="text-sm font-medium">ช่วงวันที่นิเทศ</p>
									<p class="text-sm tabular-nums">
										{cycleDateRangeLabel(
											currentBookingCycle.observationStartDate,
											currentBookingCycle.observationEndDate
										)}
									</p>
									<p class="text-xs text-muted-foreground">
										เลือกคาบได้ตั้งแต่วันเริ่มต้นถึงวันสุดท้ายของช่วงนี้
									</p>
								</div>
							</div>
							{#if cycleDateRangeError(currentBookingCycle.observationStartDate, currentBookingCycle.observationEndDate)}
								<PageState
									title="รอผู้ดูแลกำหนดช่วงวันที่นิเทศ"
									description="รอบนี้ยังไม่มีช่วงวันที่สำหรับจองคาบ ข้อมูลคำขอเดิมยังคงอ่านได้"
								/>
								<SupervisionCycleDates
									cycle={currentBookingCycle}
									allowed={canManageSchool}
									disabled={mutationBusy}
									onsave={(dates) => saveCycleDates(currentBookingCycle.id, dates)}
								/>
							{:else}
								{#key currentBookingCycle.id}<SupervisionLessonPicker
										cycle={currentBookingCycle}
										{academicYearId}
										{academicTermId}
										observations={myObservations.filter(
											(item) => item.cycleId === currentBookingCycle.id
										)}
										allowed={canRequest}
										onreloadCycle={retryManagementCycles}
										onsubmit={createBookingRequest}
									/>{/key}
							{/if}
						{:else if cyclesLoaded}<PageState
								variant="empty"
								title="ยังไม่มีรอบนิเทศที่เปิดให้จองในขณะนี้"
								description="เมื่อผู้ดูแลเปิดรับคำขอ คุณจะเลือกคาบจากตารางสอนได้ที่นี่"
							/>{/if}
						<div class="flex justify-end border-t pt-3">
							<Button
								variant="ghost"
								size="sm"
								onclick={() => {
									bookingOpen = false;
									document.getElementById('supervision-booking-toggle')?.focus();
								}}>ย่อส่วนจองคาบ</Button
							>
						</div>
					</Card.Content>
				</Card.Root>
			{/if}

			<section class="min-w-0 space-y-3" aria-labelledby="supervision-own-title">
				<h2 id="supervision-own-title" class="text-lg font-semibold">รายการของฉัน</h2>
				{#if shouldLoadObservations() && observationsReadable}
					<ToggleGroup.Root
						type="single"
						aria-label="กรองรายการของฉันตามสถานะ"
						bind:value={() => personalFilter, selectPersonalFilter}
						disabled={!observationsLoaded}
						data-testid="supervision-personal-status"
						class="grid grid-cols-2 gap-2 sm:flex sm:flex-wrap"
					>
						{#each personalResults as result (result.key)}
							<ToggleGroup.Item
								id={`supervision-personal-filter-${result.key}`}
								value={result.key}
								class="h-auto min-w-0 justify-between gap-2 px-3 py-2 text-start whitespace-normal data-[state=on]:border-primary/25 data-[state=on]:bg-primary/10 data-[state=on]:text-primary"
							>
								<span>{result.label}</span>
								<span
									data-testid={`supervision-personal-count-${result.key}`}
									class="inline-flex h-5 min-w-6 shrink-0 items-center justify-center rounded bg-muted px-1 text-xs font-semibold text-foreground tabular-nums"
								>
									{#if observationsLoaded}{result.items.length}{:else if observationsLoading}<span
											aria-hidden="true"
											class="h-2 w-3 rounded-sm bg-muted-foreground/20"
										></span>{/if}
								</span>
							</ToggleGroup.Item>
						{/each}
					</ToggleGroup.Root>
				{/if}
				<div
					class="relative space-y-3"
					data-testid="supervision-observations"
					aria-busy={observationsLoading}
				>
					{@render observationFeedback()}
					{#if observationsLoaded && !observationsReadable}<PageState
							variant="permission"
							title="ยังไม่มีสิทธิ์ดูรายการนิเทศ"
							description="ต้องมีสิทธิ์อ่านรายการตามขอบเขตที่ได้รับมอบหมาย"
						/>{/if}
					{#if (observationsLoaded || myObservations.length > 0) && observationsReadable}
						{#if myObservations.length === 0}
							<PageState
								title="ยังไม่มีรายการนิเทศของฉัน"
								description="เมื่อส่งคำขอหรือได้รับผลนิเทศ รายการจะแสดงที่นี่"
							/>
						{:else if personalResult.items.length === 0}
							<PageState
								title={personalResult.empty}
								actionLabel="ดูทุกรายการ"
								onaction={() => {
									personalFilter = 'all';
									document.getElementById('supervision-personal-filter-all')?.focus();
								}}
							/>
						{:else}
							<div class="space-y-3" data-supervision-own-list="cards">
								{#each personalResult.items as observation (observation.id)}
									{@const workflow = observationWorkflow(observation)}
									{@const canCorrect = canCorrectPersonalRequest(observation)}
									{@const returnReason = currentReturnReason(observation)}
									<div class="space-y-3 rounded-xl border bg-card p-4 sm:p-5">
										<div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
											<div class="min-w-0 space-y-1">
												<div class="flex flex-wrap items-center gap-2">
													<h3 class="text-base font-semibold wrap-anywhere">
														{observationSubjectLabel(observation)}
													</h3>
													<Badge
														variant="outline"
														class={needsPersonalAction(observation)
															? 'border-primary/25 bg-primary/10 text-primary'
															: 'bg-muted/40'}>{statusLabel(observation.status)}</Badge
													>
												</div>
												<p class="text-sm text-muted-foreground">
													{observationPeriodLabel(observation)} · {observationClassroomLabel(
														observation
													)}
												</p>
												<p class="text-xs text-muted-foreground">
													{workflow.stageLabel}{#if workflow.stage < 3 && workflow.stage >= 0 && !needsPersonalAction(observation)}
														· ถัดไป {workflow.nextOwner}{/if}
												</p>
											</div>
											<div class="shrink-0 text-sm text-muted-foreground">
												{formatDate(observation.observedAt)}
											</div>
										</div>

										{#if returnReason}
											<div
												class="rounded-md border border-primary/20 bg-primary/5 px-3 py-2 text-sm"
											>
												<p class="font-medium">เหตุผลที่ส่งกลับ</p>
												<p class="mt-1 line-clamp-3 text-muted-foreground wrap-anywhere">
													{returnReason}
												</p>
											</div>
										{/if}

										<div class="flex flex-wrap items-center justify-between gap-3">
											<div class="min-w-0">
												<p class="text-xs text-muted-foreground">ผู้นิเทศ</p>
												<p class="mt-0.5 text-sm wrap-anywhere">
													{observationEvaluatorNames(observation)}
												</p>
											</div>
											<div class="flex shrink-0 flex-wrap gap-2">
												<Button
													size="sm"
													variant={canCorrect ? 'default' : 'outline'}
													href={observationDetailHref(observation)}
													data-sveltekit-preload-data={false}
													data-sveltekit-preload-code="hover"
												>
													{#if canCorrect}{workflow.nextAction}{:else}<Eye
															class="size-4"
															aria-hidden="true"
														/>รายละเอียด{/if}
												</Button>
												{#if observation.status === 'published'}
													{@const draft = acknowledgeDrafts[observation.id]}
													{@const acknowledgeSaving =
														savingAction === `acknowledge-result:${observation.id}`}
													<Dialog.Root
														open={acknowledgeDialogId === observation.id}
														onOpenChange={(open) => setAcknowledgeDialogOpen(observation.id, open)}
													>
														<Dialog.Trigger>
															{#snippet child({ props })}
																<Button size="sm" {...props}>
																	<FileSignature class="mr-2 h-4 w-4" />
																	รับทราบผล
																</Button>
															{/snippet}
														</Dialog.Trigger>
														{#if draft}
															<Dialog.Content
																class={dialogClass}
																showCloseButton={!acknowledgeSaving}
																escapeKeydownBehavior={acknowledgeSaving ? 'ignore' : 'close'}
																interactOutsideBehavior={acknowledgeSaving ? 'ignore' : 'close'}
															>
																<Dialog.Header class={dialogHeaderClass}>
																	<Dialog.Title>รับทราบผลนิเทศ</Dialog.Title>
																	<Dialog.Description>
																		{observationSubjectLabel(observation)} · {formatDate(
																			observation.observedAt
																		)}
																	</Dialog.Description>
																</Dialog.Header>
																<form
																	class="flex min-h-0 flex-1 flex-col"
																	onsubmit={(event) => {
																		event.preventDefault();
																		void acknowledgeResult(observation.id);
																	}}
																>
																	<div class={dialogBodyClass}>
																		<Label for={`supervision-acknowledge-${observation.id}`}
																			>ความคิดเห็นเพิ่มเติม (ถ้ามี)</Label
																		>
																		<Textarea
																			id={`supervision-acknowledge-${observation.id}`}
																			bind:value={draft.comment}
																			readonly={acknowledgeSaving}
																			rows={3}
																			placeholder="ความคิดเห็นเพิ่มเติม (ถ้ามี)"
																		/>
																		{#if draft.error}
																			<p role="alert" class="text-sm text-destructive">
																				{draft.error}
																			</p>
																		{/if}
																	</div>
																	<Dialog.Footer class={dialogFooterClass}>
																		<Dialog.Close>
																			{#snippet child({ props })}<Button
																					type="button"
																					variant="outline"
																					{...props}
																					disabled={acknowledgeSaving}>ยกเลิก</Button
																				>{/snippet}
																		</Dialog.Close>
																		<LoadingButton
																			type="submit"
																			loading={acknowledgeSaving}
																			loadingLabel="กำลังบันทึก..."
																			disabled={mutationBusy}
																		>
																			ยืนยันรับทราบ
																		</LoadingButton>
																	</Dialog.Footer>
																</form>
															</Dialog.Content>
														{/if}
													</Dialog.Root>
												{/if}
											</div>
										</div>
										<details class="border-t pt-1">
											<summary
												class="cursor-pointer rounded-sm py-2 text-sm font-medium text-muted-foreground focus-visible:outline-2 focus-visible:outline-ring"
												>รอบนิเทศและข้อมูลคำขอ</summary
											>
											{@render referenceErrors(true)}
											{@render observationFactItems(observationReferenceFacts(observation))}
										</details>
									</div>
								{/each}
							</div>
						{/if}
					{/if}
				</div>
			</section>
		</div>
	{:else if section === 'requests'}
		<div data-testid="supervision-observations" aria-busy={observationsLoading} class="space-y-3">
			<Tabs.Root bind:value={queueView} onValueChange={() => (calendarReviewId = '')}>
				<Tabs.List class="h-auto w-full self-start sm:w-fit" aria-label="มุมมองคำขอจองนิเทศ">
					<Tabs.Trigger value="list" class="min-h-10 px-3"
						><Inbox aria-hidden="true" />คำขอจอง</Tabs.Trigger
					>
					<Tabs.Trigger value="calendar" class="min-h-10 px-3"
						><CalendarDays aria-hidden="true" />ปฏิทิน</Tabs.Trigger
					>
				</Tabs.List>
				{#if observationsLoaded && !observationsReadable}
					<PageState
						variant="permission"
						title="ยังไม่มีสิทธิ์ดูรายการนิเทศ"
						description="ต้องมีสิทธิ์อ่านรายการตามขอบเขตที่ได้รับมอบหมาย"
					/>
				{/if}
				<Tabs.Content value="list" class="space-y-3">
					{#if queueView === 'list' && observationsReadable}
						{@render sectionHeading(
							'คำขอจองที่รออนุมัติ',
							observationsLoaded ? requestedObservations.length : null
						)}
					{/if}
					{#if queueView === 'list'}{@render observationFeedback()}{/if}
					{#if queueView === 'list' && observationsLoaded && observationsReadable}
						{#if requestedObservations.length === 0}
							<PageState
								title="ไม่มีคำขอจองที่รออนุมัติ"
								description="เมื่อครูส่งคำขอจอง รายการจะปรากฏในส่วนนี้"
							/>
						{:else}
							<div class="space-y-3">
								{#each requestedObservations as observation (observation.id)}
									<SupervisionRequestCard
										{observation}
										cycleLabel={requestCycleLabel(observation)}
										templateLabel={requestTemplateTitle(observation)}
										cyclePending={cyclesLoading && !cyclesLoaded}
										templatePending={templatesLoading && !templatesLoaded}
										requestedLabel={formatDate(observation.requestedAt)}
									>
										{#snippet referenceFeedback()}{@render referenceErrors(true)}{/snippet}
										<SupervisionRequestReview
											{observation}
											review={requestReview}
											detailHref={observationDetailHref(observation)}
										/>
									</SupervisionRequestCard>
								{/each}
							</div>
						{/if}
					{/if}
				</Tabs.Content>
				<Tabs.Content value="calendar" class="space-y-3">
					{#if queueView === 'calendar'}{@render observationFeedback()}{/if}
					{#if queueView === 'calendar' && observationsLoaded && observationsReadable}
						{@render referenceErrors(false)}
						<SupervisionBookingCalendar
							{observations}
							{cycles}
							academicLabel={calendarAcademicLabel}
							bind:selectedDate={bookingCalendarDate}
							detailHref={observationDetailHref}
						>
							{#snippet bookingAction(observation)}
								{#if requestReview.canReview(observation)}
									<Button
										size="sm"
										variant="outline"
										disabled={mutationBusy}
										onclick={() => (calendarReviewId = observation.id)}>ตรวจคำขอ</Button
									>
								{/if}
							{/snippet}
						</SupervisionBookingCalendar>
					{/if}
				</Tabs.Content>
			</Tabs.Root>
			<Dialog.Root
				open={Boolean(calendarReviewObservation)}
				onOpenChange={(open) => {
					if (!open) calendarReviewId = '';
				}}
			>
				{#if calendarReviewObservation}
					<Dialog.Content class={cn(dialogClass, 'sm:max-w-2xl')}>
						<Dialog.Header class={dialogHeaderClass}>
							<Dialog.Title>ตรวจคำขอจองนิเทศ</Dialog.Title>
							<Dialog.Description>
								{calendarReviewObservation.observedDisplayName ?? 'ครูผู้ขอรับนิเทศ'}
							</Dialog.Description>
						</Dialog.Header>
						<div class={dialogBodyClass}>
							<SupervisionRequestReview
								observation={calendarReviewObservation}
								review={requestReview}
								showSaveEvaluators
								showSchedule
								onclose={() => (calendarReviewId = '')}
							/>
						</div>
					</Dialog.Content>
				{/if}
			</Dialog.Root>
		</div>
	{:else if section === 'evaluate'}
		<div data-testid="supervision-observations" aria-busy={observationsLoading} class="space-y-3">
			{#if !canReadObservations || !canEvaluate || (observationsLoaded && !observationsReadable)}
				{@render observationFeedback()}
				<PageState
					variant="permission"
					title="ยังไม่มีสิทธิ์ดูรายการนิเทศ"
					description="ต้องมีสิทธิ์อ่านรายการตามขอบเขตที่ได้รับมอบหมาย"
				/>
			{:else}
				<Tabs.Root bind:value={queueView}>
					<Tabs.List class="h-auto w-full self-start sm:w-fit" aria-label="มุมมองรายการประเมิน">
						<Tabs.Trigger value="list" class="min-h-10 px-3"
							><ClipboardCheck aria-hidden="true" />รายการ</Tabs.Trigger
						>
						<Tabs.Trigger value="calendar" class="min-h-10 px-3"
							><CalendarDays aria-hidden="true" />ปฏิทิน</Tabs.Trigger
						>
					</Tabs.List>
					<Tabs.Content value="list" class="space-y-4">
						{@render sectionHeading(
							'รายการที่ได้รับมอบหมายให้ประเมิน',
							observationsLoaded ? activeAssignedObservations.length : null
						)}
						{#if queueView === 'list'}{@render observationFeedback()}{/if}
						{#if observationsLoaded && (activeAssignedObservations.length || submittedAssignedObservations.length)}
							{@render referenceErrors(true)}
						{/if}
						{#if observationsLoaded && activeAssignedObservations.length === 0}
							<PageState
								title="ยังไม่มีรายการที่ต้องประเมิน"
								description="รายการจะปรากฏเมื่อผู้ดูแลอนุมัติคำขอและมอบหมายให้ประเมิน หรือเมื่อมีงานที่ยังไม่ได้ส่งผล"
							/>
						{:else if observationsLoaded}
							<div class="space-y-3" data-supervision-assigned-list="cards">
								{#each activeAssignedObservations as observation (observation.id)}
									<div
										class={cn(
											'space-y-3 rounded-xl border bg-card p-4 sm:p-5',
											evaluationObservationId === observation.id && 'border-primary/40'
										)}
									>
										<div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
											<div class="min-w-0 space-y-1">
												<div class="flex flex-wrap items-center gap-2">
													<h3 class="font-semibold">
														{observation.observedDisplayName ?? 'ครูผู้ถูกนิเทศ'}
													</h3>
													<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
													<p class="text-xs text-muted-foreground">
														{observationWorkflow(observation).stageLabel} · ถัดไป: {observationWorkflow(
															observation
														).nextOwner}
													</p>
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

										{@render observationFacts(observation)}

										<div class="flex flex-wrap items-center justify-between gap-3">
											<div class="min-w-0">
												<p class="text-xs text-muted-foreground">ผู้นิเทศร่วม</p>
												<div class="mt-1 flex flex-wrap gap-2">
													{#if observation.evaluators.length === 0}
														<Badge variant="outline">ยังไม่มอบหมาย</Badge>
													{:else}
														{#each observation.evaluators as evaluator (evaluator.id)}
															<Badge
																variant={evaluator.evaluatorUserId === currentUserId
																	? 'outline'
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
													href={observationDetailHref(observation)}
													data-sveltekit-preload-data="tap"
												>
													<Eye class="h-4 w-4" />
													รายละเอียด
												</Button>
												<Button
													type="button"
													size="sm"
													variant="default"
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
							<div class="space-y-3 pt-3">
								{@render sectionHeading(
									'ผลประเมินที่ส่งแล้ว',
									submittedAssignedObservations.length
								)}

								<div class="space-y-3" data-supervision-submitted-assigned-list="cards">
									{#each submittedAssignedObservations as observation (observation.id)}
										{@const submittedEvaluator = currentUserEvaluator(observation)}
										<div class="space-y-3 rounded-xl border bg-card p-4 sm:p-5">
											<div
												class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between"
											>
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

											{@render observationFacts(observation)}

											<div class="flex flex-wrap items-center justify-between gap-3">
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
													href={observationDetailHref(observation)}
													data-sveltekit-preload-data="tap"
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
					</Tabs.Content>
					<Tabs.Content value="calendar" class="space-y-3">
						{#if queueView === 'calendar'}{@render observationFeedback()}{/if}
						{#if queueView === 'calendar' && observationsLoaded}
							{@render referenceErrors(false)}
							<SupervisionBookingCalendar
								observations={evaluationCalendarObservations}
								{cycles}
								academicLabel={calendarAcademicLabel}
								bind:selectedDate={bookingCalendarDate}
								detailHref={observationDetailHref}
								{hasSubmittedEvaluation}
							>
								{#snippet bookingAction(observation)}
									{#if canSubmitAssignedEvaluation(observation)}
										<Button
											size="sm"
											variant="outline"
											onclick={() => prepareEvaluationDraft(observation)}
										>
											เปิดแบบประเมิน
										</Button>
									{/if}
								{/snippet}
							</SupervisionBookingCalendar>
						{/if}
					</Tabs.Content>
				</Tabs.Root>
			{/if}
		</div>
	{:else if section === 'cycles'}
		<div class="space-y-4">
			<Card.Root class="gap-0 py-0">
				<Card.Header
					class="flex flex-wrap items-baseline justify-between gap-2 border-b bg-muted/25 px-4 py-3 sm:px-6"
				>
					<h2 id="supervision-cycles-heading" class="text-base font-semibold">รายการรอบนิเทศ</h2>
					<p class="text-sm text-muted-foreground">{calendarAcademicLabel}</p>
				</Card.Header>
				<Card.Content class="px-0">
					<div
						class="relative space-y-3"
						aria-busy={cyclesLoading}
						data-testid="supervision-cycles"
					>
						{#if cyclesLoading && !cyclesLoaded}
							<div role="status" aria-label="กำลังโหลดรอบนิเทศ" class="divide-y">
								{#each [0, 1] as row (row)}
									<div class="space-y-2 px-4 py-4 sm:px-6">
										<Skeleton class="h-5 w-36 max-w-full" />
										<Skeleton class="h-4 w-56 max-w-full" />
									</div>
								{/each}
							</div>
						{/if}
						{#if cyclesError}
							<div class="px-4 py-4 sm:px-6">
								{@render managementFeedback(true, false, cyclesError, retryManagementCycles)}
							</div>
						{/if}
						{#if !routeData || cyclesLoaded}
							<div data-testid="supervision-cycles-ready">
								{#if cycles.length === 0}
									<p class="px-4 py-8 text-center text-sm text-muted-foreground sm:px-6">
										ยังไม่มีรอบนิเทศ
									</p>
								{:else}
									<ul role="list" aria-labelledby="supervision-cycles-heading" class="divide-y">
										{#each cycles as cycle (cycle.id)}
											<li
												class="grid min-w-0 gap-3 px-4 py-4 sm:px-6 xl:grid-cols-[minmax(0,5fr)_minmax(0,4fr)] xl:items-center"
											>
												<div class="min-w-0 space-y-1.5">
													<div class="flex flex-wrap items-baseline gap-x-2 gap-y-1">
														<h3 class="min-w-0 text-sm font-semibold wrap-anywhere">
															{cycle.title}
														</h3>
														<Badge variant="secondary">{statusLabel(cycle.status)}</Badge>
													</div>
													<dl class="flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
														<div class="min-w-0">
															<dt class="sr-only">ช่วงวันที่นิเทศ</dt>
															<dd class="wrap-anywhere tabular-nums">
																{cycleDateRangeLabel(
																	cycle.observationStartDate,
																	cycle.observationEndDate
																)}
															</dd>
														</div>
														<div class="min-w-0">
															<dt class="sr-only">ปีการศึกษาและภาคเรียน</dt>
															<dd class="wrap-anywhere">{cycleContextLabel(cycle)}</dd>
														</div>
													</dl>
												</div>
												{#if canManageSchool}
													<div
														role="group"
														aria-label="จัดการรอบนิเทศ"
														class="flex min-w-0 flex-wrap gap-2 xl:justify-end"
													>
														<SupervisionCycleDates
															{cycle}
															allowed={canManageSchool}
															disabled={mutationBusy}
															onsave={(dates) => saveCycleDates(cycle.id, dates)}
														/>
														<SupervisionCycleDelete
															{cycle}
															allowed={canManageSchool}
															beginObservationChange={observationWrites?.beginChange}
															ondelete={(impact) => {
																cycles = cycles.filter((item) => item.id !== impact.cycleId);
																observations = observations.filter(
																	(item) => item.cycleId !== impact.cycleId
																);
															}}
														/>
														{#if cycle.status === 'draft'}
															<LoadingButton
																size="sm"
																onclick={() => setCycleStatus(cycle, 'open')}
																loading={savingAction === `cycle-status:${cycle.id}:open`}
																loadingLabel="กำลังเปิด..."
																disabled={mutationBusy}
															>
																เปิดรับคำขอ
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
																ปิดรับคำขอ
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
																เปิดรับคำขอ
															</LoadingButton>
															<LoadingButton
																size="sm"
																variant="outline"
																onclick={() => setCycleStatus(cycle, 'archived')}
																loading={savingAction === `cycle-status:${cycle.id}:archived`}
																loadingLabel="กำลังเก็บ..."
																disabled={mutationBusy}>เก็บถาวร</LoadingButton
															>
														{/if}
													</div>
												{/if}
											</li>
										{/each}
									</ul>
								{/if}
							</div>
						{/if}
					</div>
				</Card.Content>
			</Card.Root>
		</div>
	{:else if section === 'templates'}
		<div class="space-y-4">
			<Card.Root class="gap-0 py-0">
				<Card.Header
					class="flex flex-wrap items-baseline justify-between gap-2 border-b bg-muted/25 px-4 py-3 sm:px-6"
				>
					<h2 class="text-base font-semibold">รายการแบบประเมิน</h2>
					{#if templatesLoaded}
						<p class="text-sm text-muted-foreground tabular-nums">{templateChoices.length} แบบ</p>
					{/if}
				</Card.Header>
				<Card.Content class="px-0">
					<div
						class="relative space-y-3"
						aria-busy={templatesLoading}
						data-testid="supervision-templates"
					>
						{#if routeData}{@render managementFeedback(
								templatesLoaded,
								templatesLoading,
								templatesError,
								retryManagementTemplates
							)}{/if}
						{#if !routeData || templatesLoaded}
							<div data-testid="supervision-templates-ready">
								<Table.Root aria-label="รายการแบบประเมิน">
									<Table.Header>
										<Table.Row>
											<Table.Head>ชื่อแบบประเมิน</Table.Head>
											<Table.Head class="text-center">หมวด</Table.Head>
											<Table.Head class="text-center">ข้อ</Table.Head>
											<Table.Head class="text-center">ช่วงคะแนน</Table.Head>
											<Table.Head>สถานะ</Table.Head>
											<Table.Head class="text-right">จัดการแบบประเมิน</Table.Head>
										</Table.Row>
									</Table.Header>
									<Table.Body>
										{#if templateChoices.length === 0}
											<Table.Row>
												<Table.Cell colspan={6} class="h-24 text-center text-muted-foreground">
													ยังไม่มีแบบประเมินนิเทศ
												</Table.Cell>
											</Table.Row>
										{:else}
											{#each templateChoices as template (template.id)}
												<Table.Row>
													<Table.Cell class="min-w-40 whitespace-normal font-medium"
														>{template.title}</Table.Cell
													>
													<Table.Cell class="text-center tabular-nums"
														>{'sections' in template
															? template.sections.length
															: template.sectionCount}</Table.Cell
													>
													<Table.Cell class="text-center tabular-nums"
														>{'sections' in template
															? templateItemCount(template)
															: template.itemCount}</Table.Cell
													>
													<Table.Cell class="text-center tabular-nums"
														>{template.ratingMin}–{template.ratingMax}</Table.Cell
													>
													<Table.Cell>
														<Badge variant="secondary">{templateStatusLabel(template.status)}</Badge
														>
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
							</div>
						{/if}
					</div>
				</Card.Content>
			</Card.Root>
		</div>
	{:else if section === 'overview'}
		<div class="space-y-4">
			<Card.Root>
				<Card.Content class="space-y-5 px-4 sm:px-6">
					<div
						class="relative space-y-3 rounded-lg border bg-muted/25 p-4"
						aria-busy={cyclesLoading}
						data-testid="supervision-cycle-selector"
					>
						{#if cyclesLoading && !cyclesLoaded}
							<div role="status" aria-label="กำลังโหลดรอบนิเทศ" class="space-y-2">
								<Skeleton class="h-4 w-24" />
								<Skeleton class="h-9 w-full max-w-sm" />
							</div>
						{/if}
						{@render managementFeedback(true, false, cyclesError, retryManagementCycles)}
						{#if !routeData || cyclesLoaded}
							<div class="flex flex-wrap items-end gap-3">
								<div class="min-w-0 grow space-y-2 sm:max-w-sm">
									<Label for="supervision-overview-cycle">รอบนิเทศ</Label>
									<Select.Root
										type="single"
										value={overviewCycleId}
										onValueChange={selectOverviewCycle}
									>
										<Select.Trigger
											id="supervision-overview-cycle"
											aria-label="รอบนิเทศ"
											class="w-full"
										>
											{cycles.find((cycle) => cycle.id === overviewCycleId)?.title ??
												'เลือกรอบนิเทศ'}
										</Select.Trigger>
										<Select.Content>
											{#each cycles as cycle (cycle.id)}
												<Select.Item value={cycle.id}>{cycleLabel(cycle)}</Select.Item>
											{/each}
										</Select.Content>
									</Select.Root>
								</div>
							</div>
						{/if}
					</div>
					{#if canReadSchoolReport}
						<section
							class="space-y-3"
							aria-busy={loadingProgress}
							data-testid="supervision-progress"
						>
							<h2 class="text-base font-semibold">ความคืบหน้ารอบนิเทศ</h2>
							{@render managementFeedback(true, false, progressError, () => void loadProgress())}
							{#if progress || loadingProgress}
								{@render summaryMetrics(
									[
										{ label: 'รายการทั้งหมด', value: progress && progress.totalObservations },
										{ label: 'เสร็จสิ้น', value: progress && progress.completedCount },
										{ label: 'รอครูรับทราบ', value: progress && progress.publishedCount },
										{
											label: 'คะแนนเฉลี่ย',
											value: progress ? (progress.averageRating?.toFixed(2) ?? '—') : null
										}
									],
									'สรุปผลรอบนิเทศ'
								)}
							{/if}
							{#if progress}
								<div class="space-y-2">
									<div class="flex items-center justify-between text-sm">
										<span>ความคืบหน้ารวม</span>
										<span>{progressPercent}%</span>
									</div>
									<Progress value={progressPercent} aria-label="ความคืบหน้ารวม" />
								</div>
							{:else if loadingProgress}
								<div role="status" aria-label="กำลังโหลดความคืบหน้ารอบนิเทศ" class="space-y-2">
									<Skeleton class="h-5 w-36" />
									<Skeleton class="h-2 w-full" />
								</div>
							{/if}
						</section>
					{/if}
					<div
						class="relative space-y-3"
						aria-busy={loadingTeacherStatus}
						data-testid="supervision-teacher-status"
					>
						<h2 class="text-base font-semibold">สถานะครูในรอบนิเทศ</h2>
						{#if routeData}{@render managementFeedback(
								teacherStatusLoaded,
								loadingTeacherStatus,
								teacherStatusError,
								loadTeacherStatusOverview
							)}{/if}
						{#if !routeData || teacherStatusLoaded}
							<div class="rounded-lg border" data-testid="supervision-teacher-status-ready">
								<Table.Root aria-label="สถานะครูในรอบนิเทศ">
									<Table.Header>
										<Table.Row>
											<Table.Head>ครู</Table.Head>
											<Table.Head>กลุ่มสาระ</Table.Head>
											<Table.Head>คาบนิเทศ</Table.Head>
											<Table.Head>สถานะครู</Table.Head>
											<Table.Head>ขั้นตอนถัดไป</Table.Head>
											<Table.Head>ผู้ประเมิน</Table.Head>
											<Table.Head class="text-right">คะแนน</Table.Head>
										</Table.Row>
									</Table.Header>
									<Table.Body>
										{#if teacherStatusRows.length === 0}
											<Table.Row>
												<Table.Cell colspan={7} class="h-24 text-center text-muted-foreground">
													{overviewCycleId
														? 'ไม่มีข้อมูลสถานะครูในรอบนี้'
														: 'เลือกรอบนิเทศเพื่อดูสถานะครู'}
												</Table.Cell>
											</Table.Row>
										{:else}
											{#each teacherStatusRows as row (row.teacherId)}
												<Table.Row>
													<Table.Cell class="font-medium">{row.teacherDisplayName}</Table.Cell>
													<Table.Cell>
														{row.organizationUnitNames.length > 0
															? row.organizationUnitNames.join(', ')
															: '-'}
													</Table.Cell>
													<Table.Cell>
														<div class="space-y-1">
															<p class="font-medium">{row.lessonTitle ?? '-'}</p>
															<p class="text-xs text-muted-foreground">
																{formatDate(row.observedAt)}
															</p>
														</div>
													</Table.Cell>
													<Table.Cell>
														<Badge variant="secondary">
															{row.status ? statusLabel(row.status) : 'ยังไม่จอง'}
														</Badge>
													</Table.Cell>
													<Table.Cell>{row.nextStepLabel}</Table.Cell>
													<Table.Cell>
														{row.evaluatorNames.length > 0 ? row.evaluatorNames.join(', ') : '-'}
													</Table.Cell>
													<Table.Cell class="text-right">
														{row.averageRating === null || row.averageRating === undefined
															? '-'
															: row.averageRating.toFixed(2)}
													</Table.Cell>
												</Table.Row>
											{/each}
										{/if}
									</Table.Body>
								</Table.Root>
							</div>
						{/if}
					</div>
				</Card.Content>
			</Card.Root>
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={evaluationDialogOpen} onOpenChange={setEvaluationDialogOpen}>
	<Dialog.Content class={cn(dialogClass, 'sm:max-w-5xl')}>
		<Dialog.Header class={dialogHeaderClass}>
			<Dialog.Title>ทำแบบประเมินนิเทศ</Dialog.Title>
			<Dialog.Description>
				{selectedEvaluation?.observedDisplayName ?? 'ครูผู้ถูกนิเทศ'}
				{#if selectedEvaluationTemplate}
					· {selectedEvaluationTemplate.title}{/if}
			</Dialog.Description>
		</Dialog.Header>

		<form
			class="flex min-h-0 flex-1 flex-col"
			onsubmit={(event) => {
				event.preventDefault();
				void saveEvaluation();
			}}
		>
			<div class={dialogBodyClass} aria-busy={evaluationTemplateLoading}>
				{@render managementFeedback(
					!!selectedEvaluationTemplate,
					evaluationTemplateLoading,
					evaluationTemplateError,
					() => void loadEvaluationTemplate()
				)}
				{#if selectedEvaluation && selectedEvaluationTemplate && !evaluationTemplateLoading && !evaluationTemplateError}
					<dl
						class="grid grid-cols-2 gap-3 rounded-lg border border-primary/15 bg-primary/5 p-4 text-sm md:grid-cols-4"
					>
						<div>
							<dt class="text-xs text-muted-foreground">คะแนนรวม</dt>
							<dd class="text-lg font-semibold tabular-nums">
								{selectedEvaluationDraftSummary.totalScore} / {selectedEvaluationDraftSummary.maxScore}
							</dd>
						</div>
						<div>
							<dt class="text-xs text-muted-foreground">ร้อยละ</dt>
							<dd class="text-lg font-semibold tabular-nums">
								{selectedEvaluationDraftSummary.percentage === null
									? '—'
									: selectedEvaluationDraftSummary.percentage.toFixed(2)}
							</dd>
						</div>
						<div>
							<dt class="text-xs text-muted-foreground">ระดับคุณภาพ</dt>
							<dd class="text-lg font-semibold">{selectedEvaluationDraftSummary.qualityLabel}</dd>
						</div>
						<div>
							<dt class="text-xs text-muted-foreground">ข้อคะแนนที่ตอบแล้ว</dt>
							<dd class="text-lg font-semibold tabular-nums">
								{selectedEvaluationDraftSummary.answeredRatingCount} /
								{selectedEvaluationDraftSummary.ratingItemCount}
							</dd>
						</div>
					</dl>

					<dl class="grid grid-cols-2 gap-x-4 gap-y-2 text-sm sm:grid-cols-3">
						{#each observationLessonFacts(selectedEvaluation) as detail (detail.label)}
							<div class="min-w-0">
								<dt class="text-xs text-muted-foreground">{detail.label}</dt>
								<dd class="wrap-anywhere font-medium">{detail.value}</dd>
							</div>
						{/each}
					</dl>
					<details class="rounded-lg border px-3">
						<summary
							class="cursor-pointer rounded-sm py-2 text-sm font-medium focus-visible:outline-2 focus-visible:outline-ring"
							>รายละเอียดคาบนิเทศ</summary
						>
						{@render observationFacts(selectedEvaluation, true)}
					</details>

					{#each selectedEvaluationRubricSections as section (section.localId)}
						{@const progress = sectionRubricProgress(
							section,
							responseDrafts,
							selectedEvaluationTemplate.ratingMax
						)}
						<section
							class="rounded-lg border bg-card"
							aria-labelledby={`evaluation-section-${section.localId}`}
						>
							<div
								class="flex flex-wrap items-start justify-between gap-3 rounded-t-lg border-b bg-muted/25 p-4"
							>
								<div>
									<h3 id={`evaluation-section-${section.localId}`} class="text-base font-semibold">
										{section.title}
									</h3>
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
							<div class="divide-y px-4">
								{#each section.items as item (item.localId)}
									<div id={`evaluation-field-${item.localId}`} class="space-y-3 py-4">
										<div class="space-y-1">
											{#if item.itemType === 'rating'}
												<p id={`evaluation-label-${item.localId}`} class="text-sm font-medium">
													{item.label}
												</p>
											{:else}
												<Label for={`evaluation-answer-${item.localId}`}>{item.label}</Label>
											{/if}
											{#if item.required}<p
													id={`evaluation-required-${item.localId}`}
													class="text-xs text-muted-foreground"
												>
													บังคับตอบ
												</p>{/if}
											{#if item.description}
												<p
													id={`evaluation-description-${item.localId}`}
													class="text-xs text-muted-foreground"
												>
													{item.description}
												</p>
											{/if}
										</div>
										{#if item.itemType === 'rating'}
											<div
												role="group"
												aria-labelledby={`evaluation-label-${item.localId}`}
												aria-describedby={cn(
													item.required && `evaluation-required-${item.localId}`,
													item.description && `evaluation-description-${item.localId}`
												) || undefined}
												class="flex flex-wrap gap-2"
											>
												{#each ratingScale(selectedEvaluationTemplate.ratingMin, selectedEvaluationTemplate.ratingMax) as score, scoreIndex (score)}
													<Button
														id={scoreIndex === 0 ? `evaluation-answer-${item.localId}` : undefined}
														type="button"
														size="sm"
														aria-pressed={responseDrafts[item.localId]?.ratingScore ===
															String(score)}
														variant={responseDrafts[item.localId]?.ratingScore === String(score)
															? 'default'
															: 'outline'}
														onclick={() =>
															updateDraft(item.localId, { ratingScore: String(score) })}
													>
														{score}
													</Button>
												{/each}
											</div>
										{:else}
											<Textarea
												id={`evaluation-answer-${item.localId}`}
												aria-required={item.required}
												aria-describedby={item.description
													? `evaluation-description-${item.localId}`
													: undefined}
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
						</section>
					{/each}
					<p class="text-sm text-muted-foreground">
						เมื่อผู้ประเมินที่จำเป็นส่งผลครบ ระบบจะแจ้งให้ครูรับทราบผล
					</p>
				{:else if !evaluationTemplateLoading && !evaluationTemplateError}
					<PageState
						title="ยังไม่ได้เลือกรายการประเมิน"
						description="เลือกงานนิเทศที่ได้รับมอบหมายก่อนเริ่มทำแบบประเมิน"
					/>
				{/if}
			</div>
			{#if evaluationFormError || (evaluationValidationAttempted && missingRequiredEvaluationItems.length > 0)}<p
					role="alert"
					class="px-4 pb-2 text-sm text-destructive sm:px-6"
				>
					{evaluationFormError ||
						`กรอกหัวข้อบังคับให้ครบ (${missingRequiredEvaluationItems.length} ข้อ)`}
				</p>{/if}
			<Dialog.Footer class={dialogFooterClass}>
				<Button
					type="button"
					variant="outline"
					onclick={clearEvaluationDraft}
					disabled={savingEvaluation !== null}
				>
					ปิด
				</Button>
				<LoadingButton
					type="submit"
					loading={savingEvaluation !== null}
					loadingLabel="กำลังส่ง..."
					disabled={!selectedEvaluation ||
						!selectedEvaluationTemplate ||
						evaluationTemplateLoading ||
						!!evaluationTemplateError}
				>
					ส่งผลประเมิน
				</LoadingButton>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root
	open={createCycleDialogOpen}
	onOpenChange={(open) => {
		if (savingAction !== 'create-cycle') createCycleDialogOpen = open;
	}}
>
	<Dialog.Content
		class={cn(dialogClass, 'sm:max-w-2xl')}
		onEscapeKeydown={(event) => {
			if (savingAction === 'create-cycle') event.preventDefault();
		}}
		onInteractOutside={(event) => {
			if (savingAction === 'create-cycle') event.preventDefault();
		}}
	>
		<Dialog.Header class={dialogHeaderClass}>
			<Dialog.Title>สร้างรอบนิเทศ</Dialog.Title>
			<Dialog.Description>
				{academicYearId ? academicYearLabel(academicYearId) : 'ยังไม่ได้เลือกปีการศึกษา'} · {academicTermLabel(
					academicTermId
				)}
			</Dialog.Description>
		</Dialog.Header>
		<form
			class="flex min-h-0 flex-1 flex-col"
			onsubmit={(event) => {
				event.preventDefault();
				void createCycle();
			}}
		>
			<div class={dialogBodyClass}>
				<fieldset
					disabled={savingAction === 'create-cycle'}
					class="grid min-w-0 gap-4 sm:grid-cols-2"
				>
					<div class="space-y-2 sm:col-span-2">
						<Label for="supervision-cycle-title">ชื่อรอบนิเทศ</Label>
						<Input
							id="supervision-cycle-title"
							bind:value={cycleForm.title}
							placeholder="เช่น นิเทศการสอน ภาคเรียนที่ 1"
						/>
					</div>
					<div class="space-y-2 sm:col-span-2">
						<Label for="supervision-cycle-template">แบบประเมิน</Label>
						{#if routeData}{@render managementFeedback(
								templatesLoaded,
								templatesLoading,
								templatesError,
								retryManagementTemplates
							)}{/if}
						{#if templatesLoaded && templateChoices.length === 0}
							<p class="text-sm text-muted-foreground">
								สร้างแบบประเมินก่อนจึงจะเลือกมาใช้ในรอบนิเทศได้
							</p>
						{:else if !routeData || templatesLoaded}
							<Select.Root type="single" bind:value={cycleForm.templateId}>
								<Select.Trigger id="supervision-cycle-template" class="w-full">
									{templateChoices.find((template) => template.id === cycleForm.templateId)
										?.title ?? 'เลือกแบบประเมิน'}
								</Select.Trigger>
								<Select.Content>
									{#each templateChoices as template (template.id)}
										<Select.Item value={template.id}>{template.title}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						{/if}
					</div>
					<div class="space-y-2">
						<Label for="supervision-cycle-start">วันเริ่มนิเทศ</Label><DatePicker
							id="supervision-cycle-start"
							ariaLabel="วันเริ่มนิเทศ"
							value={cycleForm.observationStartDate || undefined}
							onValueChange={(value) => (cycleForm.observationStartDate = value ?? '')}
							required
						/>
					</div>
					<div class="space-y-2">
						<Label for="supervision-cycle-end">วันสุดท้ายที่นิเทศ</Label><DatePicker
							id="supervision-cycle-end"
							ariaLabel="วันสุดท้ายที่นิเทศ"
							value={cycleForm.observationEndDate || undefined}
							onValueChange={(value) => (cycleForm.observationEndDate = value ?? '')}
							minValue={cycleForm.observationStartDate || undefined}
							required
						/>
					</div>
					<div class="space-y-2 sm:col-span-2">
						<Label for="supervision-cycle-status">สถานะรอบ</Label>
						<Select.Root type="single" bind:value={cycleForm.status}>
							<Select.Trigger id="supervision-cycle-status" class="w-full">
								{cycleStatusCreateOptions.find((option) => option.value === cycleForm.status)
									?.label ?? 'เลือกสถานะรอบ'}
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
							{cycleStatusCreateOptions.find((option) => option.value === cycleForm.status)
								?.description}
						</p>
					</div>
					<div class="space-y-2 sm:col-span-2">
						<Label for="supervision-cycle-description">รายละเอียด (ถ้ามี)</Label>
						<Textarea
							id="supervision-cycle-description"
							bind:value={cycleForm.description}
							rows={2}
							placeholder="รายละเอียดเพิ่มเติม"
						/>
					</div>
				</fieldset>
			</div>
			{#if cycleFormError}<p role="alert" class="px-4 pb-2 text-sm text-destructive sm:px-6">
					{cycleFormError}
				</p>{/if}
			<Dialog.Footer class={dialogFooterClass}>
				<Button
					type="button"
					variant="outline"
					disabled={savingAction === 'create-cycle'}
					onclick={() => (createCycleDialogOpen = false)}>ยกเลิก</Button
				>
				<LoadingButton
					type="submit"
					disabled={routeData &&
						(!templatesLoaded ||
							templatesLoading ||
							!!templatesError ||
							templateChoices.length === 0)}
					loading={savingAction === 'create-cycle'}
					loadingLabel="กำลังสร้าง..."
				>
					สร้างรอบนิเทศ
				</LoadingButton>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root open={createTemplateDialogOpen} onOpenChange={setTemplateEditorDialogOpen}>
	<Dialog.Content class={cn(dialogClass, 'sm:max-w-5xl')}>
		<Dialog.Header class={dialogHeaderClass}>
			<Dialog.Title
				>{editingTemplateId ? 'แก้ไขแบบประเมินนิเทศ' : 'สร้างแบบประเมินนิเทศ'}</Dialog.Title
			>
			<Dialog.Description>
				กำหนดช่วงคะแนนและเพิ่มหัวข้อที่ผู้นิเทศต้องตอบในแต่ละหมวด
			</Dialog.Description>
		</Dialog.Header>
		<form
			class="flex min-h-0 flex-1 flex-col"
			onsubmit={(event) => {
				event.preventDefault();
				void createTemplate();
			}}
		>
			<div class={dialogBodyClass} aria-busy={templateDetailLoading}>
				{#if routeData}{@render managementFeedback(
						templateDetailLoaded,
						templateDetailLoading,
						templateDetailError,
						() => {
							void loadManagementTemplateDetail('edit', editingTemplateId);
						}
					)}{/if}
				{#if !routeData || templateDetailLoaded}
					<section class="space-y-3" aria-labelledby="supervision-template-settings">
						<h2 id="supervision-template-settings" class="text-base font-semibold">
							ข้อมูลแบบประเมิน
						</h2>
						<div class="grid min-w-0 gap-4 rounded-lg border bg-card p-4 md:grid-cols-3">
							<div class="min-w-0 space-y-2 md:col-span-3">
								<Label for="supervision-template-title">ชื่อแบบประเมิน</Label>
								<Input
									id="supervision-template-title"
									bind:value={templateForm.title}
									placeholder="ชื่อแบบประเมิน"
								/>
							</div>
							<div class="min-w-0 space-y-2">
								<Label for="supervision-template-status">สถานะ</Label>
								<Select.Root type="single" bind:value={templateForm.status}>
									<Select.Trigger id="supervision-template-status" class="w-full">
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
								<Label for="supervision-template-minimum">คะแนนต่ำสุด</Label>
								<Input
									id="supervision-template-minimum"
									type="number"
									bind:value={templateForm.ratingMin}
								/>
							</div>
							<div class="min-w-0 space-y-2">
								<Label for="supervision-template-maximum">คะแนนสูงสุด</Label>
								<Input
									id="supervision-template-maximum"
									type="number"
									bind:value={templateForm.ratingMax}
								/>
							</div>
							<div class="min-w-0 space-y-2 md:col-span-3">
								<Label for="supervision-template-description">รายละเอียด (ถ้ามี)</Label>
								<Textarea
									id="supervision-template-description"
									bind:value={templateForm.description}
									rows={2}
								/>
							</div>
						</div>
					</section>

					<div class="flex flex-wrap items-center justify-between gap-3 border-t pt-4">
						<div>
							<h2 class="text-base font-semibold">โครงสร้างแบบประเมิน</h2>
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
							<Button type="button" variant="outline" size="sm" onclick={addTemplateSection}>
								<Plus class="mr-2 h-4 w-4" />
								เพิ่มหมวด
							</Button>
						</div>
					</div>

					<div class="space-y-3">
						{#each templateForm.sections as section, sectionIndex (section.localId)}
							<div class="min-w-0 rounded-lg border bg-card">
								<div class="space-y-3 rounded-t-lg border-b bg-muted/25 p-4">
									<div class="flex flex-col gap-3 lg:flex-row lg:items-end lg:justify-between">
										<div class="min-w-0 flex-1 space-y-2">
											<Label for={`supervision-section-title-${section.localId}`}>ชื่อหมวด</Label>
											<Input
												id={`supervision-section-title-${section.localId}`}
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
										<Label for={`supervision-section-description-${section.localId}`}
											>คำอธิบายหมวด (ถ้ามี)</Label
										>
										<Input
											id={`supervision-section-description-${section.localId}`}
											value={section.description}
											oninput={(event) =>
												updateTemplateSection(section.localId, {
													description: (event.currentTarget as HTMLInputElement).value
												})}
										/>
									</div>
								</div>

								<div class="divide-y px-4">
									{#if section.items.length === 0}
										<PageState
											title="หมวดนี้ยังไม่มีหัวข้อ"
											description="เพิ่มหัวข้อแบบคะแนนหรือข้อเสนอแนะ"
										/>
									{:else}
										{#each section.items as item, itemIndex (item.localId)}
											<div class="min-w-0 py-4">
												<div
													class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
												>
													<div class="flex min-w-0 flex-wrap items-center gap-3">
														<Badge variant="secondary">
															{item.itemType === 'rating' ? 'คะแนน' : 'ข้อความ'}
														</Badge>
														<div class="flex items-center gap-2">
															<Checkbox
																id={`supervision-item-required-${item.localId}`}
																checked={item.required}
																onCheckedChange={(checked) =>
																	updateTemplateItem(section.localId, item.localId, {
																		required: !!checked
																	})}
															/>
															<Label
																for={`supervision-item-required-${item.localId}`}
																class="text-xs text-muted-foreground">บังคับตอบ</Label
															>
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
														<Label for={`supervision-item-title-${item.localId}`}
															>หัวข้อประเมิน</Label
														>
														<Input
															id={`supervision-item-title-${item.localId}`}
															value={item.label}
															oninput={(event) =>
																updateTemplateItem(section.localId, item.localId, {
																	label: (event.currentTarget as HTMLInputElement).value
																})}
														/>
													</div>
													<div class="min-w-0 space-y-2">
														<Label for={`supervision-item-description-${item.localId}`}
															>คำอธิบาย (ถ้ามี)</Label
														>
														<Input
															id={`supervision-item-description-${item.localId}`}
															value={item.description}
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

									<div class="flex flex-wrap gap-2 py-3">
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
				{/if}
			</div>
			{#if templateFormError}<p role="alert" class="px-4 pb-2 text-sm text-destructive sm:px-6">
					{templateFormError}
				</p>{/if}
			<Dialog.Footer class={dialogFooterClass}>
				<Button type="button" variant="outline" onclick={() => setTemplateEditorDialogOpen(false)}
					>ยกเลิก</Button
				>
				<LoadingButton
					type="submit"
					disabled={routeData &&
						(!templateDetailLoaded || templateDetailLoading || !!templateDetailError)}
					loading={savingTemplate}
					loadingLabel={editingTemplateId ? 'กำลังบันทึก...' : 'กำลังสร้าง...'}
				>
					{editingTemplateId ? 'บันทึกแบบประเมิน' : 'สร้างแบบประเมิน'}
				</LoadingButton>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={previewTemplateDialogOpen} onOpenChange={setTemplatePreviewDialogOpen}>
	<Dialog.Content class={cn(dialogClass, 'sm:max-w-5xl')}>
		<Dialog.Header class={dialogHeaderClass}>
			<Dialog.Title>ตัวอย่างแบบประเมินนิเทศ</Dialog.Title>
			<Dialog.Description>ดูหัวข้อและช่วงคะแนนก่อนนำแบบประเมินไปใช้ในรอบนิเทศ</Dialog.Description>
		</Dialog.Header>

		<div class={cn(dialogBodyClass, 'bg-muted/25')} aria-busy={templateDetailLoading}>
			{#if routeData}{@render managementFeedback(
					templateDetailLoaded,
					templateDetailLoading,
					templateDetailError,
					() => {
						void loadManagementTemplateDetail('preview', previewTemplateId);
					}
				)}{/if}
			{#if previewTemplate}
				<div
					class="mx-auto max-w-4xl rounded-lg border bg-card p-4 sm:p-6"
					data-template-preview-mode="readonly"
				>
					<div class="space-y-3">
						<div>
							<h2 class="text-xl font-semibold">{previewTemplate.title}</h2>
							{#if previewTemplate.description}
								<p class="mt-1 text-sm text-muted-foreground">{previewTemplate.description}</p>
							{/if}
						</div>
						<div class="flex flex-wrap gap-2">
							<Badge variant="secondary">{templateStatusLabel(previewTemplate.status)}</Badge>
							<Badge variant="outline">{previewTemplate.sections.length} หมวด</Badge>
							<Badge variant="outline">{templateItemCount(previewTemplate)} ข้อ</Badge>
							<Badge variant="outline">
								คะแนน {previewTemplate.ratingMin} - {previewTemplate.ratingMax}
							</Badge>
						</div>
					</div>

					<div class="mt-5 rounded-lg border">
						<Table.Root class="min-w-lg" aria-label="หัวข้อและช่วงคะแนนในแบบประเมิน">
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
			{:else if !templateDetailLoading && !templateDetailError}
				<PageState title="ไม่พบแบบประเมิน" description="เลือกแบบประเมินจากรายการอีกครั้ง" />
			{/if}
		</div>

		<Dialog.Footer class={dialogFooterClass}>
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
