<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import SupervisionCycleDates from '#lib/components/supervision/SupervisionCycleDates.svelte';
	import SupervisionRequestAvailability from '#lib/components/supervision/SupervisionRequestAvailability.svelte';
	import SupervisionRequestCard from '#lib/components/supervision/SupervisionRequestCard.svelte';
	import SupervisionBookingCalendar from '#lib/components/supervision/SupervisionBookingCalendar.svelte';
	import { cycleDateRangeError, cycleDateRangeLabel } from '#lib/supervision/booking-range.js';
	import SupervisionCycleDelete from '#lib/components/supervision/SupervisionCycleDelete.svelte';
	import SupervisionLessonPicker from '#lib/components/supervision/SupervisionLessonPicker.svelte';
	import type { SupervisionLessonSelection } from '#lib/supervision/booking-range.js';
	import { savedLessonLabels } from '#lib/supervision/supervision-presentation.js';
	import {
		supervisionStatusLabel as statusLabel,
		observationWorkflow
	} from '#lib/supervision/supervision-presentation.js';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { SupervisionWorkspaceRouteData } from '#lib/supervision/workspace-route-data.js';
	import {
		BarChart3,
		Check,
		ChevronsUpDown,
		ArrowDown,
		ArrowUp,
		Eye,
		FileSignature,
		Plus,
		Settings2,
		Trash2
	} from '@lucide/svelte';
	import { toast } from 'svelte-sonner';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import { academicContextualMenuPath } from '#lib/academic-context/route-context.js';
	import { isAbortError, LatestRequest } from '#lib/async/latest-request.js';
	import {
		acknowledgeSupervisionObservation,
		approveSupervisionObservationRequest,
		createSupervisionCycle,
		createSupervisionTemplate,
		getSupervisionEvaluatorAvailability,
		getSupervisionTemplate,
		listSupervisionTemplateSummaries,
		type SupervisionTemplateSummary,
		getSupervisionCycleProgress,
		getSupervisionTeacherStatusOverview,
		listSupervisionCycles,
		listSupervisionObservations,
		requestSupervisionObservation,
		returnSupervisionObservationRequest,
		submitMySupervisionEvaluation,
		updateSupervisionCycle,
		updateSupervisionTemplate,
		type CreateSupervisionCycleRequest,
		type CreateSupervisionTemplateRequest,
		type SaveEvaluationRequest,
		type SupervisionCycle,
		type SupervisionCycleStatus,
		type SupervisionCycleProgress,
		type SupervisionEvaluatorAvailability,
		type SupervisionEvaluatorAvailabilityResponse,
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
	import * as Command from '#lib/components/ui/command/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import { Progress } from '#lib/components/ui/progress/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';

	export type SupervisionWorkspaceSection =
		'mine' | 'evaluate' | 'requests' | 'cycles' | 'templates' | 'overview' | 'approvals';

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
	};
	type CycleFormState = {
		title: string;
		description: string;
		templateId: string;
		status: SupervisionCycleStatus;
		observationStartDate: string;
		observationEndDate: string;
	};

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
			title: 'นิเทศการสอน',
			description: 'จองคาบนิเทศ ดูรายการของฉัน และรับทราบผลนิเทศ'
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
		},
		approvals: {
			title: 'รับรองและอนุมัติผลนิเทศ',
			description: 'ตรวจผลประเมิน รับรองผล และอนุมัติผลนิเทศขั้นสุดท้าย'
		}
	};

	let {
		section,
		routeData
	}: { section: SupervisionWorkspaceSection; routeData: SupervisionWorkspaceRouteData } = $props();
	const academicContext = getAcademicContextStore();
	const academicYearId = $derived(
		routeData ? routeData.academicYearId : $academicContext.selected.academicYearId
	);
	const academicTermId = $derived(
		routeData ? routeData.academicTermId : $academicContext.selected.academicTermId
	);
	const academicContextOptions = $derived($academicContext.options);

	const managementCyclesRequest = new LatestRequest();
	const managementTemplatesRequest = new LatestRequest();
	const templateDetailRequest = new LatestRequest();
	let managementContext = '';
	let observedCycleUrl = '';
	let renderedTeacherCycleId = '';
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
	let requestsView = $state('requests');
	let bookingCalendarDate = $state('');
	let bookingOpen = $state(false);
	let evaluationTemplateLoading = $state(false);
	let evaluationTemplateError = $state('');
	let progressError = $state('');
	let requestEvaluatorErrors = $state<Record<string, string>>({});
	let saving = $state(false);
	let savingAction = $state<string | null>(null);
	let savingTemplate = $state(false);
	let savingEvaluation = $state<'submit' | null>(null);
	let evaluationDialogOpen = $state(false);
	let cycles = $state<SupervisionCycle[]>([]);
	let templates = $state<SupervisionTemplate[]>([]);
	let observations = $state<SupervisionObservation[]>([]);
	let requestEvaluatorAvailability = $state<
		Record<string, SupervisionEvaluatorAvailabilityResponse>
	>({});
	let requestEvaluatorAvailabilityLoading = $state<Record<string, boolean>>({});
	let selectedCycleId = $state('');
	let requestEvaluatorIds = $state<Record<string, string[]>>({});
	let requestReturnComments = $state<Record<string, string>>({});
	let evaluatorPickerOpenByRequest = $state<Record<string, boolean>>({});
	let evaluationObservationId = $state('');
	let responseDrafts = $state<{ [itemId: string]: ResponseDraft }>({});
	let acknowledgeComment = $state('');
	let progressCycleId = $state('');
	let progress = $state<SupervisionCycleProgress | null>(null);
	let teacherStatusRows = $state<SupervisionTeacherStatusRow[]>([]);
	let loadingProgress = $state(false);
	let loadingTeacherStatus = $state(false);
	let createCycleDialogOpen = $state(false);
	let cycleFormError = $state('');
	let createTemplateDialogOpen = $state(false);
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
	const evaluatorAvailabilityRequests: Record<string, LatestRequest> = {};
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
	const referenceFeedbackVisible = $derived(
		Boolean(
			(routeData.cycles && ((cyclesLoading && !cyclesLoaded) || cyclesError)) ||
			(routeData.templates && ((templatesLoading && !templatesLoaded) || templatesError))
		)
	);
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
	const canReadSchoolReport = $derived(
		$can.hasAny(
			PERMISSIONS.SUPERVISION_READ_SCHOOL,
			PERMISSIONS.SUPERVISION_MANAGE_SCHOOL,
			PERMISSIONS.SUPERVISION_APPROVE_SCHOOL
		)
	);
	const canReport = $derived(
		$can.has(PERMISSIONS.SUPERVISION_READ_SCHOOL) ||
			canManageSchool ||
			canManageRequests ||
			canApprove
	);
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
	const assignedObservations = $derived(
		observations.filter((observation) =>
			observation.evaluators.some((evaluator) => evaluator.evaluatorUserId === currentUserId)
		)
	);
	const activeAssignedObservations = $derived(
		assignedObservations.filter(canSubmitAssignedEvaluation)
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

	const certifiableObservations = $derived(
		canManageRequests ? observations.filter((item) => item.status === 'evaluators_submitted') : []
	);

	const approvableObservations = $derived(
		canApprove ? observations.filter((item) => item.status === 'approved') : []
	);

	const approvalWorkflowObservations = $derived([
		...certifiableObservations,
		...approvableObservations
	]);

	const progressPercent = $derived(
		progress && progress.totalObservations > 0
			? Math.round((progress.completedCount / progress.totalObservations) * 100)
			: 0
	);

	let managementRevision = 0;
	let mutationLifetime = 0;
	onDestroy(() => {
		mutationLifetime += 1;
		for (const request of Object.values(evaluatorAvailabilityRequests)) request.abort();
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
				for (const request of Object.values(evaluatorAvailabilityRequests)) request.abort();
				savingAction = null;
				savingTemplate = false;
				cycles = [];
				templates = [];
				templateSummaries = [];
				teacherStatusRows = [];
				observations = [];
				observationsLoaded = false;
				bookingCalendarDate = '';
				bookingOpen = false;
				savingEvaluation = null;
				clearEvaluationDraft();
				requestEvaluatorAvailability = {};
				requestEvaluatorAvailabilityLoading = {};
				requestEvaluatorErrors = {};
				evaluatorPickerOpenByRequest = {};
				requestEvaluatorIds = {};
				requestReturnComments = {};
				progress = null;
				progressError = '';
				loadingProgress = false;
				cyclesLoaded = false;
				templatesLoaded = false;
				teacherStatusLoaded = false;
				renderedTeacherCycleId = '';
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
				progressCycleId = source.cycleId;
			}
			observationsLoading = source.observations !== null;
			observationsError = '';
			observedCycleUrl = source.cycleId;
			cyclesLoading = source.cycles !== null;
			templatesLoading = source.templates !== null;
			loadingTeacherStatus = source.teacherStatus !== null;
			cyclesError = '';
			templatesError = '';
			teacherStatusError = '';
			const observationRead = observationRequest.begin();
			if (source.observations)
				void Promise.all([source.identity, source.observations]).then(([owner, result]) => {
					if (
						owner !== appIdentityKey() ||
						key !== currentManagementContext() ||
						!observationRequest.isCurrent(observationRead.revision)
					)
						return;
					observationsLoading = false;
					if (!result.ok) {
						observationsError = result.error;
						return;
					}
					observations = result.data.items;
					observationsReadable = result.data.readable;
					observationsLoaded = true;
				});
			const cycleRead = managementCyclesRequest.begin();
			const templateRead = managementTemplatesRequest.begin();
			const statusRead = teacherStatusRequest.begin();
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

					progressCycleId = selected;
					if (section === 'overview' && source.cycleId && selected !== source.cycleId) {
						teacherStatusRequest.abort();
						void loadTeacherStatusOverview();
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
					progressCycleId = result.data.cycleId;
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
			progressCycleId = cycles.some((item) => item.id === selected)
				? selected
				: (cycles[0]?.id ?? '');
			void loadTeacherStatusOverview();
		});
	});

	function selectOverviewCycle(id: string) {
		if (!routeData) {
			progressCycleId = id;
			return;
		}
		observedCycleUrl = id;
		progressCycleId = id;
		const url = new URL(page.url.href);

		if (id) url.searchParams.set('cycleId', id);
		else url.searchParams.delete('cycleId');

		goto(resolve(`staff/academic/supervision/overview?${url.searchParams.toString()}`), {
			shallow: true,
			state: page.state
		});
		void loadTeacherStatusOverview();
	}

	async function retryManagementCycles() {
		if (
			!academicYearId ||
			(section === 'approvals' && !canReadSchoolReport) ||
			(section === 'evaluate' && !canReadObservations)
		)
			return;

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
			if (
				section === 'overview' &&
				(!cycles.some((item) => item.id === progressCycleId) || !teacherStatusLoaded)
			) {
				progressCycleId = cycles[0]?.id ?? '';
				void loadTeacherStatusOverview();
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

	function observationLessonTitle(observation: SupervisionObservation): string {
		return savedLessonLabels(observation).subject;
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

	function sectionRoute(target: SupervisionWorkspaceSection): string {
		const path =
			target === 'mine' ? '/staff/academic/supervision' : `/staff/academic/supervision/${target}`;
		return academicContextualMenuPath(
			path,
			{ academicYearId, academicTermId },
			academicContextOptions
		);
	}

	const sectionLinks = $derived(
		(
			[
				{ key: 'mine', label: 'ของฉัน', visible: true },
				{ key: 'evaluate', label: 'ประเมิน', visible: canEvaluate },
				{ key: 'requests', label: 'คำขอจอง', visible: canManageRequests },
				{ key: 'overview', label: 'ภาพรวม', visible: canReport },
				{ key: 'approvals', label: 'รับรอง/อนุมัติ', visible: canReport },
				{ key: 'cycles', label: 'รอบนิเทศ', visible: canManageSchool },
				{ key: 'templates', label: 'แบบประเมิน', visible: canManageSchool }
			] satisfies { key: SupervisionWorkspaceSection; label: string; visible: boolean }[]
		).filter((item) => item.visible)
	);

	function shouldLoadObservations() {
		return routeData.observations !== null;
	}
	async function retryObservations() {
		if (!academicYearId || !canReadObservations) return;
		const { revision, signal } = observationRequest.begin();
		const owner = currentManagementContext();
		observationsLoading = true;
		observationsError = '';
		try {
			const items = await listSupervisionObservations(
				{ academicYearId, ...(academicTermId ? { academicTermId } : {}) },
				{ signal }
			);

			if (owner !== currentManagementContext() || !observationRequest.isCurrent(revision)) return;
			observations = items;
			observationsReadable = true;
			observationsLoaded = true;
		} catch (error) {
			if (
				!isAbortError(error) &&
				owner === currentManagementContext() &&
				observationRequest.isCurrent(revision)
			)
				observationsError = error instanceof Error ? error.message : 'โหลดรายการนิเทศไม่สำเร็จ';
		} finally {
			if (owner === currentManagementContext() && observationRequest.isCurrent(revision))
				observationsLoading = false;
		}
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
		progressCycleId ||= cycle.id;
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
		observationsLoaded = true;
		observations = observations.some((item) => item.id === observation.id)
			? observations.map((item) => (item.id === observation.id ? observation : item))
			: [observation, ...observations];
	}

	function currentUserEvaluator(observation: SupervisionObservation) {
		return observation.evaluators.find((evaluator) => evaluator.evaluatorUserId === currentUserId);
	}

	function canSubmitAssignedEvaluation(observation: SupervisionObservation): boolean {
		const evaluator = currentUserEvaluator(observation);
		return Boolean(
			evaluator &&
			evaluator.status !== 'submitted' &&
			(observation.status === 'planned' ||
				observation.status === 'in_progress' ||
				(observation.status === 'evaluators_submitted' && !evaluator.isRequired))
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
		const response = await requestSupervisionObservation({
			cycleId: currentBookingCycle.id,
			academicTermId,
			...selection
		});
		if (!ownsMutation() || !canRequest) return;
		replaceObservation(requireMutationData(response, 'ส่งคำขอไม่สำเร็จ'));
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

	function selectedRequestEvaluatorIds(observationId: string): string[] {
		return requestEvaluatorIds[observationId] ?? [];
	}

	function requestEvaluatorOptions(observationId: string): SupervisionEvaluatorAvailability[] {
		return requestEvaluatorAvailability[observationId]?.items ?? [];
	}

	function availableRequestEvaluatorOptions(
		observationId: string
	): SupervisionEvaluatorAvailability[] {
		return requestEvaluatorOptions(observationId).filter((evaluator) => evaluator.available);
	}

	function unavailableRequestEvaluatorCount(observationId: string): number {
		return requestEvaluatorOptions(observationId).filter((evaluator) => !evaluator.available)
			.length;
	}

	function selectedRequestEvaluators(observationId: string): SupervisionEvaluatorAvailability[] {
		const selectedIds = new Set(selectedRequestEvaluatorIds(observationId));
		return requestEvaluatorOptions(observationId).filter((staff) => selectedIds.has(staff.id));
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

	async function loadRequestEvaluatorAvailability(observationId: string) {
		if (
			!canManageRequests ||
			requestEvaluatorAvailabilityLoading[observationId] ||
			!requestedObservations.some((item) => item.id === observationId && item.canManage)
		)
			return;
		const context = currentManagementContext();
		const request = evaluatorAvailabilityRequests[observationId] ?? new LatestRequest();
		evaluatorAvailabilityRequests[observationId] = request;
		const { revision, signal } = request.begin();
		const ownsRead = () => request.isCurrent(revision) && context === currentManagementContext();
		requestEvaluatorErrors = { ...requestEvaluatorErrors, [observationId]: '' };
		requestEvaluatorAvailabilityLoading = {
			...requestEvaluatorAvailabilityLoading,
			[observationId]: true
		};
		try {
			const result = await getSupervisionEvaluatorAvailability(observationId, { signal });
			if (!ownsRead()) return;
			const availableIds = new Set(
				result.items.filter((evaluator) => evaluator.available).map((evaluator) => evaluator.id)
			);
			requestEvaluatorAvailability = {
				...requestEvaluatorAvailability,
				[observationId]: result
			};
			requestEvaluatorIds = {
				...requestEvaluatorIds,
				[observationId]: selectedRequestEvaluatorIds(observationId).filter((id) =>
					availableIds.has(id)
				)
			};
		} catch (error) {
			if (isAbortError(error)) return;
			if (ownsRead())
				requestEvaluatorErrors = {
					...requestEvaluatorErrors,
					[observationId]:
						error instanceof Error ? error.message : 'ไม่สามารถตรวจสอบผู้ประเมินที่ว่างได้'
				};
		} finally {
			if (ownsRead()) {
				requestEvaluatorAvailabilityLoading = {
					...requestEvaluatorAvailabilityLoading,
					[observationId]: false
				};
			}
		}
	}

	function handleRequestEvaluatorPickerOpen(observationId: string, open: boolean) {
		setRequestEvaluatorPickerOpen(observationId, open);
		if (open) void loadRequestEvaluatorAvailability(observationId);
	}

	function requestAvailabilityUnverified(observationId: string): boolean {
		return Boolean(
			requestEvaluatorAvailabilityLoading[observationId] || requestEvaluatorErrors[observationId]
		);
	}

	function toggleRequestEvaluatorForRequest(
		observationId: string,
		staff: SupervisionEvaluatorAvailability
	) {
		if (!canManageRequests || requestAvailabilityUnverified(observationId) || !staff.available)
			return;
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
		evaluatorAvailabilityRequests[observationId]?.abort();
		const { [observationId]: _evaluatorIds, ...remainingEvaluatorIds } = requestEvaluatorIds;
		const { [observationId]: _comment, ...remainingComments } = requestReturnComments;
		const { [observationId]: _pickerOpen, ...remainingPickerOpen } = evaluatorPickerOpenByRequest;
		const { [observationId]: _availability, ...remainingAvailability } =
			requestEvaluatorAvailability;
		const { [observationId]: _availabilityLoading, ...remainingAvailabilityLoading } =
			requestEvaluatorAvailabilityLoading;
		requestEvaluatorIds = remainingEvaluatorIds;
		requestReturnComments = remainingComments;
		evaluatorPickerOpenByRequest = remainingPickerOpen;
		requestEvaluatorAvailability = remainingAvailability;
		requestEvaluatorAvailabilityLoading = remainingAvailabilityLoading;
	}

	async function approveRequest(observationId: string) {
		if (!canManageRequests) return;
		const current = observations.find((item) => item.id === observationId);
		if (
			!current?.canManage ||
			current.evaluators.some((evaluator) => evaluator.status === 'submitted')
		)
			return;
		if (requestAvailabilityUnverified(observationId)) {
			toast.error('รอตรวจช่วงคาบและผู้ประเมินให้สำเร็จก่อน');
			return;
		}
		const availability = requestEvaluatorAvailability[observationId];
		if (availability && (!availability.withinCycleDates || availability.overlap.blocked)) {
			toast.error('ช่วงคาบนี้ยังอนุมัติไม่ได้ ตรวจช่วงวันที่และรายการที่ทับกันก่อน');
			return;
		}
		const evaluatorIds = selectedRequestEvaluatorIds(observationId);
		if (evaluatorIds.length === 0) {
			toast.error('เลือกผู้ประเมินอย่างน้อย 1 คนก่อนอนุมัติ');
			return;
		}

		savingAction = `approve-request:${observationId}`;
		const ownsMutation = captureMutationOwner();
		try {
			const response = await approveSupervisionObservationRequest(observationId, {
				evaluators: evaluatorIds.map((evaluatorId) => ({
					evaluatorUserId: evaluatorId,
					isRequired: true
				}))
			});
			if (!ownsMutation()) return;
			const observation = requireMutationData(response, 'อนุมัติคำขอไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('อนุมัติคำขอและมอบหมายผู้ประเมินแล้ว');
			clearRequestApprovalState(observationId);
		} catch (error) {
			if (!ownsMutation()) return;
			void loadRequestEvaluatorAvailability(observationId);
			toast.error(error instanceof Error ? error.message : 'อนุมัติคำขอไม่สำเร็จ');
		} finally {
			if (ownsMutation()) savingAction = null;
		}
	}

	async function returnRequest(id: string) {
		if (!canManageRequests) return;
		const current = observations.find((item) => item.id === id);
		if (!current || current.evaluators.some((evaluator) => evaluator.status === 'submitted'))
			return;
		savingAction = `return-request:${id}`;
		const ownsMutation = captureMutationOwner();
		try {
			const response = await returnSupervisionObservationRequest(id, {
				comment: requestReturnComments[id] || null
			});
			if (!ownsMutation()) return;
			const observation = requireMutationData(response, 'ส่งกลับคำขอไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('ส่งกลับคำขอแล้ว');
			clearRequestApprovalState(id);
		} catch (error) {
			if (!ownsMutation()) return;
			toast.error(error instanceof Error ? error.message : 'ส่งกลับคำขอไม่สำเร็จ');
		} finally {
			if (ownsMutation()) savingAction = null;
		}
	}

	function prepareEvaluationDraft(observation: SupervisionObservation) {
		if (!canEvaluate || !canReadObservations) return;
		if (!canSubmitAssignedEvaluation(observation)) return;
		evaluationObservationId = observation.id;
		responseDrafts = {};
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

	async function saveEvaluation() {
		if (
			!canEvaluate ||
			!selectedEvaluation ||
			!canSubmitAssignedEvaluation(selectedEvaluation) ||
			evaluationTemplateLoading ||
			evaluationTemplateError ||
			!selectedEvaluationTemplate
		)
			return;
		if (!evaluationObservationId) {
			toast.error('เลือกรายการประเมินก่อน');
			return;
		}
		const missing = missingRequiredEvaluationLabels();
		if (missing.length > 0) {
			toast.error(`กรอกหัวข้อบังคับให้ครบ (${missing.length} ข้อ)`);
			return;
		}

		savingEvaluation = 'submit';
		const ownsMutation = captureMutationOwner();
		const observationId = evaluationObservationId;
		try {
			const payload = evaluationPayload();
			const response = await submitMySupervisionEvaluation(observationId, payload);
			if (!ownsMutation()) return;
			const observation = requireMutationData(response, 'บันทึกผลประเมินไม่สำเร็จ');
			replaceObservation(observation);
			if (evaluationObservationId === observationId) clearEvaluationDraft();
			toast.success('ส่งผลประเมินแล้ว');
		} catch (error) {
			if (!ownsMutation()) return;
			toast.error(error instanceof Error ? error.message : 'บันทึกผลประเมินไม่สำเร็จ');
		} finally {
			if (ownsMutation()) savingEvaluation = null;
		}
	}

	async function acknowledgeResult(id: string) {
		savingAction = `acknowledge-result:${id}`;
		const ownsMutation = captureMutationOwner();
		try {
			const response = await acknowledgeSupervisionObservation(id, {
				comment: acknowledgeComment || null
			});
			if (!ownsMutation()) return;
			const observation = requireMutationData(response, 'รับทราบผลไม่สำเร็จ');
			replaceObservation(observation);
			toast.success('รับทราบผลนิเทศแล้ว');
			acknowledgeComment = '';
		} catch (error) {
			if (!ownsMutation()) return;
			toast.error(error instanceof Error ? error.message : 'รับทราบผลไม่สำเร็จ');
		} finally {
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
		const ownsMutation = captureMutationOwner();
		const workflowRevision = templateWorkflowRevision;
		const submittedTemplateId = editingTemplateId;
		try {
			const payload = templatePayload();
			const response = submittedTemplateId
				? await updateSupervisionTemplate(submittedTemplateId, payload)
				: await createSupervisionTemplate(payload);
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
			toast.error(error instanceof Error ? error.message : 'บันทึกแบบประเมินไม่สำเร็จ');
		} finally {
			if (ownsMutation()) savingTemplate = false;
		}
	}

	async function loadProgress() {
		if (!canReadSchoolReport) return;
		if (!progressCycleId) {
			toast.error('เลือกรอบนิเทศก่อน');
			return;
		}

		const cycleId = progressCycleId;
		const { revision, signal } = progressRequest.begin();
		progressError = '';
		loadingProgress = true;
		try {
			const nextProgress = await getSupervisionCycleProgress(cycleId, { signal });
			if (!progressRequest.isCurrent(revision)) return;
			progress = nextProgress;
		} catch (error) {
			if (isAbortError(error)) return;
			if (progressRequest.isCurrent(revision))
				progressError = error instanceof Error ? error.message : 'โหลดรายงานไม่สำเร็จ';
		} finally {
			if (progressRequest.isCurrent(revision)) loadingProgress = false;
		}
	}

	async function loadTeacherStatusOverview() {
		if (!canReport) return;
		const cycleId = progressCycleId;
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
				cycleId !== progressCycleId ||
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

<PageShell
	title={sectionConfig.title}
	description={sectionConfig.description}
	contentClass="flex flex-col gap-4 [&>*]:mt-0"
>
	{#snippet actions()}
		{#if section === 'mine' && canRequest && !bookingOpen}
			<Button size="sm" onclick={() => (bookingOpen = true)}
				><Plus class="size-4" />เปิดจองคาบนิเทศ</Button
			>
		{/if}
		{#if canManageSchool && section === 'cycles'}
			<Button size="sm" onclick={openCreateCycleDialog}>
				<Plus class="mr-2 h-4 w-4" />
				สร้างรอบนิเทศ
			</Button>
		{/if}
		{#if canManageSchool && section === 'templates'}
			<Button size="sm" variant="outline" onclick={openCreateTemplateDialog}>
				<Settings2 class="mr-2 h-4 w-4" />
				สร้างแบบประเมิน
			</Button>
		{/if}
	{/snippet}

	<nav aria-label="เมนูนิเทศการสอน" class="flex gap-1 overflow-x-auto border-b pb-2">
		{#each sectionLinks as item (item.key)}
			<Button
				href={sectionRoute(item.key)}
				data-sveltekit-preload-data={item.key !== 'overview' ? false : 'tap'}
				variant={section === item.key ? 'secondary' : 'ghost'}
				aria-current={section === item.key ? 'page' : undefined}
				class="shrink-0"
				size="sm"
			>
				{item.label}
			</Button>
		{/each}
	</nav>

	{#if section === 'mine'}
		<div class="grid grid-cols-2 gap-3 sm:grid-cols-4" data-testid="supervision-personal-status">
			{#each [{ label: 'รออนุมัติคำขอ', statuses: ['requested'] }, { label: 'ต้องแก้ไขและส่งใหม่', statuses: ['returned'] }, { label: 'รอฉันรับทราบ', statuses: ['published'] }, { label: 'เสร็จสิ้น', statuses: ['completed', 'acknowledged'] }] as metric (metric.label)}
				<div class="rounded-xl border bg-background p-4">
					<p class="text-2xl font-semibold">
						{observationsLoaded && observationsReadable
							? myObservations.filter((item) => metric.statuses.includes(item.status)).length
							: '—'}
					</p>
					<p class="mt-1 text-xs text-muted-foreground">{metric.label}</p>
				</div>
			{/each}
		</div>
	{:else}
		<div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold">
					{!cyclesLoaded ||
					(section === 'approvals' && !canReadSchoolReport) ||
					(section === 'evaluate' && !canReadObservations)
						? '—'
						: cycles.length}
				</p>
				<p class="text-xs text-muted-foreground">รอบนิเทศ</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold">
					{shouldLoadObservations() && observationsLoaded && observationsReadable
						? requestedObservations.length
						: '—'}
				</p>
				<p class="text-xs text-muted-foreground">คำขอรออนุมัติ</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold">
					{shouldLoadObservations() && observationsLoaded && observationsReadable
						? activeAssignedObservations.length
						: '—'}
				</p>
				<p class="text-xs text-muted-foreground">รอประเมิน</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold">
					{templatesLoaded && canReadObservations ? templateSummaries.length : '—'}
				</p>
				<p class="text-xs text-muted-foreground">แบบประเมิน</p>
			</div>
		</div>
	{/if}

	{#if ['mine', 'requests', 'evaluate', 'approvals'].includes(section)}
		<div
			class="relative space-y-2"
			hidden={!referenceFeedbackVisible}
			data-testid="supervision-references"
		>
			{#if routeData.cycles}{@render managementFeedback(
					cyclesLoaded,
					cyclesLoading,
					cyclesError,
					retryManagementCycles
				)}{/if}
			{#if routeData.templates}{@render managementFeedback(
					templatesLoaded,
					templatesLoading,
					templatesError,
					retryManagementTemplates
				)}{/if}
		</div>
	{/if}
	{#if section === 'mine'}
		<div class="space-y-4">
			{#if bookingOpen && canRequest}
				<Card.Root class="overflow-hidden border-primary/20">
					<Card.Header class="flex flex-row items-start justify-between gap-3 bg-muted/20">
						<div>
							<Card.Title>จองคาบนิเทศของฉัน</Card.Title><Card.Description
								>หนึ่งคำขอสำหรับหนึ่งคาบหรือหลายคาบต่อเนื่องของห้องเดียวกัน</Card.Description
							>
						</div>
						<Button variant="ghost" size="sm" onclick={() => (bookingOpen = false)}
							>ปิดหน้าจอง</Button
						>
					</Card.Header>
					<Card.Content class="space-y-4 pt-4">
						{#if cyclesLoaded && currentBookingCycle && academicYearId && academicTermId}
							<div class="flex flex-wrap items-center gap-3 rounded-lg bg-muted/40 p-3">
								<Label for="booking-cycle">รอบนิเทศ</Label><Select.Root
									type="single"
									value={currentBookingCycle.id}
									onValueChange={(value) => (selectedCycleId = value)}
									><Select.Trigger
										id="booking-cycle"
										class="w-full sm:w-72"
										aria-label="เลือกรอบนิเทศ">{currentBookingCycle.title}</Select.Trigger
									><Select.Content
										>{#each activeBookingCycles as cycle (cycle.id)}<Select.Item value={cycle.id}
												>{cycle.title}</Select.Item
											>{/each}</Select.Content
									></Select.Root
								>
								<p class="text-xs text-muted-foreground">
									เปิดรับคำขอจนกว่าผู้ดูแลจะเปลี่ยนสถานะรอบนิเทศ
								</p>
							</div>
							<div
								class="rounded-lg border border-primary/20 bg-primary/5 p-4"
								data-testid="supervision-cycle-dates"
							>
								<p class="font-medium">
									ช่วงวันที่นิเทศ · {cycleDateRangeLabel(
										currentBookingCycle.observationStartDate,
										currentBookingCycle.observationEndDate
									)}
								</p>
								<p class="mt-1 text-sm text-muted-foreground">
									รวมวันเริ่มต้นและวันสุดท้าย สถานะรับคำขอยังคงเปลี่ยนโดยผู้ดูแล
								</p>
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
					</Card.Content>
				</Card.Root>
			{/if}

			<Card.Root>
				<Card.Header>
					<Card.Title>รายการของฉัน</Card.Title>
				</Card.Header>
				<Card.Content>
					<div
						class="relative"
						data-testid="supervision-observations"
						aria-busy={observationsLoading}
					>
						{@render managementFeedback(
							observationsLoaded,
							observationsLoading,
							observationsError,
							retryObservations
						)}
						{#if observationsLoaded && !observationsReadable}<PageState
								variant="permission"
								title="ยังไม่มีสิทธิ์ดูรายการนิเทศ"
								description="ต้องมีสิทธิ์อ่านรายการตามขอบเขตที่ได้รับมอบหมาย"
							/>{/if}
						{#if observationsLoaded && observationsReadable}
							{#if myObservations.length === 0}
								<PageState
									title="ยังไม่มีรายการนิเทศของฉัน"
									description="เมื่อส่งคำขอหรือได้รับผลนิเทศ รายการจะแสดงที่นี่"
								/>
							{:else}
								<div class="space-y-3" data-supervision-own-list="cards">
									{#each myObservations as observation (observation.id)}
										<div class="rounded-md border bg-background p-4">
											<div
												class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between"
											>
												<div class="min-w-0 space-y-1">
													<div class="flex flex-wrap items-center gap-2">
														<h3 class="font-semibold">{observationSubjectLabel(observation)}</h3>
														<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
														<p class="text-xs text-muted-foreground">
															{observationWorkflow(observation).stageLabel} · ถัดไป: {observationWorkflow(
																observation
															).nextOwner}
														</p>
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
														href={observationDetailHref(observation)}
														data-sveltekit-preload-data="tap"
													>
														<Eye class="h-4 w-4" />
														{observation.status === 'returned' &&
														canRequest &&
														!observation.evaluators.some(
															(evaluator) => evaluator.status === 'submitted'
														)
															? observationWorkflow(observation).nextAction
															: 'รายละเอียด'}
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
																		loading={savingAction ===
																			`acknowledge-result:${observation.id}`}
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
						{/if}
					</div></Card.Content
				>
			</Card.Root>
		</div>
	{:else if section === 'requests'}
		<div data-testid="supervision-observations" aria-busy={observationsLoading} class="space-y-3">
			<Tabs.Root
				bind:value={requestsView}
				onValueChange={(view) => {
					if (view === 'calendar') evaluatorPickerOpenByRequest = {};
				}}
			>
				<Tabs.List class="self-start" aria-label="มุมมองคำขอจองนิเทศ">
					<Tabs.Trigger value="requests">คำขอจอง</Tabs.Trigger>
					<Tabs.Trigger value="calendar">ปฏิทิน</Tabs.Trigger>
				</Tabs.List>
				{@render managementFeedback(
					observationsLoaded,
					observationsLoading,
					observationsError,
					retryObservations
				)}
				{#if observationsLoaded && !observationsReadable}
					<PageState
						variant="permission"
						title="ยังไม่มีสิทธิ์ดูรายการนิเทศ"
						description="ต้องมีสิทธิ์อ่านรายการตามขอบเขตที่ได้รับมอบหมาย"
					/>
				{/if}
				<Tabs.Content value="requests" class="space-y-3">
					{#if requestsView === 'requests' && observationsLoaded && observationsReadable}
						<h2 class="text-lg font-semibold">คำขอจองที่รออนุมัติ</h2>
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
										requestedLabel={formatDate(observation.requestedAt)}
									>
										{#if observation.canManage}<SupervisionRequestAvailability
												result={requestEvaluatorAvailability[observation.id]}
												loading={!!requestEvaluatorAvailabilityLoading[observation.id]}
												error={requestEvaluatorErrors[observation.id] ?? ''}
												showError={!requestEvaluatorPickerOpen(observation.id)}
												disabled={mutationBusy}
												oninspect={() => void loadRequestEvaluatorAvailability(observation.id)}
											/>{/if}
										<div class="grid gap-4 border-t pt-3 lg:grid-cols-[minmax(0,1fr)_320px]">
											<div class="space-y-2">
												<Label>ผู้ประเมิน</Label>
												<div
													class="flex min-h-10 flex-wrap items-center gap-2 rounded-md border p-2"
												>
													{#if selectedRequestEvaluators(observation.id).length === 0}
														<span class="text-sm text-muted-foreground"
															>ยังไม่ได้เลือกผู้ประเมิน</span
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
													onOpenChange={(open) =>
														handleRequestEvaluatorPickerOpen(observation.id, open)}
												>
													<Popover.Trigger
														disabled={!observation.canManage ||
															mutationBusy ||
															requestEvaluatorAvailabilityLoading[observation.id]}
													>
														{#snippet child({ props })}
															<Button
																type="button"
																variant="outline"
																role="combobox"
																aria-label="เพิ่ม/เลือกผู้ประเมิน"
																aria-expanded={requestEvaluatorPickerOpen(observation.id)}
																class="w-full justify-between font-normal sm:w-[320px]"
																{...props}
															>
																<span class="truncate">
																	{requestEvaluatorAvailabilityLoading[observation.id] &&
																	!requestEvaluatorAvailability[observation.id]
																		? 'กำลังตรวจผู้ประเมินที่ว่าง...'
																		: 'เพิ่ม/เลือกผู้ประเมิน'}
																</span>
																<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
															</Button>
														{/snippet}
													</Popover.Trigger>
													<Popover.Content class="w-[--bits-popover-trigger-width] p-0">
														{@render managementFeedback(
															!!requestEvaluatorAvailability[observation.id],
															!!requestEvaluatorAvailabilityLoading[observation.id],
															requestEvaluatorErrors[observation.id] ?? '',
															() => void loadRequestEvaluatorAvailability(observation.id)
														)}
														<Command.Root>
															<Command.Input placeholder="ค้นหาครูผู้ประเมิน..." />
															{#if requestEvaluatorAvailability[observation.id]}<Command.Empty
																	>ไม่พบครูผู้ประเมินที่ว่าง</Command.Empty
																>{/if}
															{#if requestEvaluatorAvailability[observation.id]}<Command.List
																	class="max-h-72"
																>
																	<Command.Group heading="พร้อมประเมินตลอดช่วงคาบ">
																		{#each availableRequestEvaluatorOptions(observation.id) as evaluator (evaluator.id)}
																			<Command.Item
																				value={`${evaluator.name} ${evaluator.title ?? ''} ${evaluator.id}`}
																				disabled={requestAvailabilityUnverified(observation.id)}
																				onSelect={() =>
																					toggleRequestEvaluatorForRequest(
																						observation.id,
																						evaluator
																					)}
																			>
																				<Check
																					class={cn(
																						'mr-2 h-4 w-4',
																						selectedRequestEvaluatorIds(observation.id).includes(
																							evaluator.id
																						)
																							? 'opacity-100'
																							: 'opacity-0'
																					)}
																				/>
																				<span>{evaluator.name}</span>
																			</Command.Item>
																		{/each}
																	</Command.Group>
																	{#if unavailableRequestEvaluatorCount(observation.id)}<Command.Group
																			heading="ไม่ว่างในช่วงคาบนี้"
																			>{#each requestEvaluatorOptions(observation.id).filter((item) => !item.available) as evaluator (evaluator.id)}<Command.Item
																					value={`${evaluator.name} ${evaluator.id}`}
																					disabled
																					><div class="min-w-0 space-y-1">
																						<p>{evaluator.name}</p>
																						<p class="text-xs text-muted-foreground">
																							{evaluator.conflictReason ?? 'มีงานทับช่วงคาบ'}
																						</p>
																						{#if evaluator.conflict}<p
																								class="text-xs text-muted-foreground"
																							>
																								{evaluator.conflict.observedDisplayName ??
																									'รายการนิเทศ'} · {formatDate(
																									evaluator.conflict.observedAt
																								)}{#if evaluator.conflict.lessonTitle}
																									· {evaluator.conflict.lessonTitle}{/if}
																							</p>{/if}
																					</div></Command.Item
																				>{/each}</Command.Group
																		>{/if}
																</Command.List>{/if}
														</Command.Root>
													</Popover.Content>
												</Popover.Root>
												{#if unavailableRequestEvaluatorCount(observation.id) > 0}
													<p class="text-xs text-muted-foreground">
														ผู้ประเมินไม่ว่าง {unavailableRequestEvaluatorCount(observation.id)} คน ดูเหตุผลในกลุ่ม
														“ไม่ว่างในช่วงคาบนี้”
													</p>
												{/if}
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

										<div class="flex flex-wrap items-center justify-end gap-2">
											<Button
												size="sm"
												variant="outline"
												class="h-8"
												href={observationDetailHref(observation)}
												data-sveltekit-preload-data="tap"
											>
												<Eye class="h-4 w-4" />
												รายละเอียด
											</Button>
											<LoadingButton
												variant="outline"
												loading={savingAction === `return-request:${observation.id}`}
												loadingLabel="กำลังส่งกลับ..."
												disabled={mutationBusy ||
													observation.evaluators.some(
														(evaluator) => evaluator.status === 'submitted'
													)}
												onclick={() => returnRequest(observation.id)}
											>
												ส่งกลับคำขอ
											</LoadingButton>
											<LoadingButton
												onclick={() => approveRequest(observation.id)}
												loading={savingAction === `approve-request:${observation.id}`}
												loadingLabel="กำลังอนุมัติ..."
												disabled={!observation.canManage ||
													mutationBusy ||
													requestAvailabilityUnverified(observation.id) ||
													observation.evaluators.some(
														(evaluator) => evaluator.status === 'submitted'
													) ||
													selectedRequestEvaluatorIds(observation.id).length === 0 ||
													requestEvaluatorAvailability[observation.id]?.overlap.blocked ||
													requestEvaluatorAvailability[observation.id]?.withinCycleDates === false}
											>
												อนุมัติและมอบหมาย
											</LoadingButton>
										</div>
									</SupervisionRequestCard>
								{/each}
							</div>
						{/if}
					{/if}
				</Tabs.Content>
				<Tabs.Content value="calendar">
					{#if requestsView === 'calendar' && observationsLoaded && observationsReadable}
						<SupervisionBookingCalendar
							{observations}
							bind:selectedDate={bookingCalendarDate}
							detailHref={observationDetailHref}
						/>
					{/if}
				</Tabs.Content>
			</Tabs.Root>
		</div>
	{:else if section === 'evaluate'}
		<div class="space-y-4">
			<Card.Root>
				<Card.Header>
					<Card.Title>รายการที่ได้รับมอบหมายให้ประเมิน</Card.Title>
				</Card.Header>
				<Card.Content class="space-y-4">
					<div
						class="relative"
						data-testid="supervision-observations"
						aria-busy={observationsLoading}
					>
						{@render managementFeedback(
							observationsLoaded,
							observationsLoading,
							observationsError,
							retryObservations
						)}
						{#if observationsLoaded && !observationsReadable}<PageState
								variant="permission"
								title="ยังไม่มีสิทธิ์ดูรายการนิเทศ"
								description="ต้องมีสิทธิ์อ่านรายการตามขอบเขตที่ได้รับมอบหมาย"
							/>{/if}
						{#if observationsLoaded && observationsReadable}
							{#if activeAssignedObservations.length === 0}
								<PageState
									title="ยังไม่มีรายการที่ต้องประเมิน"
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
											<div
												class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between"
											>
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
														href={observationDetailHref(observation)}
														data-sveltekit-preload-data="tap"
													>
														<Eye class="h-4 w-4" />
														รายละเอียด
													</Button>
													<Button
														type="button"
														size="sm"
														variant={evaluationObservationId === observation.id
															? 'default'
															: 'outline'}
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
										<h3 class="text-sm font-semibold">ผลประเมินที่ส่งแล้ว</h3>
										<p class="text-xs text-muted-foreground">
											รายการที่ส่งผลประเมินแล้วจะเก็บไว้ตรวจสอบย้อนหลัง ไม่แสดงปนกับคิวที่ต้องทำ
										</p>
									</div>

									<div class="space-y-3" data-supervision-submitted-assigned-list="cards">
										{#each submittedAssignedObservations as observation (observation.id)}
											{@const submittedEvaluator = currentUserEvaluator(observation)}
											<div class="rounded-md border bg-muted/20 p-4">
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
						{/if}
					</div></Card.Content
				>
			</Card.Root>
		</div>
	{:else if section === 'cycles'}
		<div class="space-y-4">
			<Card.Root>
				<Card.Header class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
					<div>
						<Card.Title>รอบนิเทศ</Card.Title>
						<Card.Description>รอบนิเทศเชื่อมกับปีการศึกษาและภาคเรียนของระบบวิชาการ</Card.Description
						>
					</div>
					{#if canManageSchool}
						<Button onclick={openCreateCycleDialog}>
							<Plus class="mr-2 h-4 w-4" />
							สร้างรอบนิเทศ
						</Button>
					{/if}
				</Card.Header>
				<Card.Content>
					<div
						class="relative space-y-3"
						aria-busy={cyclesLoading}
						data-testid="supervision-cycles"
					>
						{#if routeData}{@render managementFeedback(
								cyclesLoaded,
								cyclesLoading,
								cyclesError,
								retryManagementCycles
							)}{/if}
						{#if !routeData || cyclesLoaded}
							<div data-testid="supervision-cycles-ready">
								<Table.Root>
									<Table.Header>
										<Table.Row>
											<Table.Head>รอบนิเทศ</Table.Head>
											<Table.Head>ภาคเรียน</Table.Head>

											<Table.Head>สถานะ</Table.Head>
											<Table.Head class="text-right">คำสั่ง</Table.Head>
										</Table.Row>
									</Table.Header>
									<Table.Body>
										{#if cycles.length === 0}
											<Table.Row>
												<Table.Cell colspan={4} class="h-24 text-center text-muted-foreground">
													ยังไม่มีรอบนิเทศ
												</Table.Cell>
											</Table.Row>
										{:else}
											{#each cycles as cycle (cycle.id)}
												<Table.Row>
													<Table.Cell class="whitespace-normal"
														><p class="font-medium">{cycle.title}</p>
														<p class="mt-1 text-xs text-muted-foreground">
															{cycleDateRangeLabel(
																cycle.observationStartDate,
																cycle.observationEndDate
															)}
														</p></Table.Cell
													>
													<Table.Cell>
														{cycleContextLabel(cycle)}
													</Table.Cell>
													<Table.Cell
														><Badge variant="secondary">{statusLabel(cycle.status)}</Badge
														></Table.Cell
													>
													<Table.Cell class="text-right">
														<div class="flex flex-wrap justify-end gap-2">
															<SupervisionCycleDates
																{cycle}
																allowed={canManageSchool}
																disabled={mutationBusy}
																onsave={(dates) => saveCycleDates(cycle.id, dates)}
															/>
															<SupervisionCycleDelete
																{cycle}
																allowed={canManageSchool}
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
															{:else}
																<span class="text-sm text-muted-foreground">-</span>
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
	{:else if section === 'templates'}
		<div class="space-y-4">
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
										{#if templateChoices.length === 0}
											<Table.Row>
												<Table.Cell colspan={6} class="h-24 text-center text-muted-foreground">
													ยังไม่มีแบบประเมินนิเทศ
												</Table.Cell>
											</Table.Row>
										{:else}
											{#each templateChoices as template (template.id)}
												<Table.Row>
													<Table.Cell class="font-medium">{template.title}</Table.Cell>
													<Table.Cell
														>{'sections' in template
															? template.sections.length
															: template.sectionCount}</Table.Cell
													>
													<Table.Cell
														>{'sections' in template
															? templateItemCount(template)
															: template.itemCount}</Table.Cell
													>
													<Table.Cell>{template.ratingMin} - {template.ratingMax}</Table.Cell>
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
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<BarChart3 class="h-5 w-5" />
						สถานะครูในรอบนิเทศ
					</Card.Title>
					<Card.Description>
						ดูว่าครูแต่ละคนจองคาบแล้วหรือยัง อยู่ขั้นตอนไหน ใครเป็นผู้นิเทศ และคะแนนล่าสุดเท่าไร
					</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-4">
					<div
						class="relative space-y-3"
						aria-busy={cyclesLoading}
						data-testid="supervision-cycle-selector"
					>
						{#if routeData}{@render managementFeedback(
								cyclesLoaded,
								cyclesLoading,
								cyclesError,
								retryManagementCycles
							)}{/if}
						{#if !routeData || cyclesLoaded}
							<div class="flex flex-col gap-2 md:flex-row">
								<Select.Root
									type="single"
									value={progressCycleId}
									onValueChange={selectOverviewCycle}
								>
									<Select.Trigger aria-label="รอบนิเทศ" class="w-full md:w-[360px]">
										{cycles.find((cycle) => cycle.id === progressCycleId)?.title ?? 'เลือกรอบนิเทศ'}
									</Select.Trigger>
									<Select.Content>
										{#each cycles as cycle (cycle.id)}
											<Select.Item value={cycle.id}>{cycleLabel(cycle)}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
								<LoadingButton
									onclick={loadTeacherStatusOverview}
									loading={loadingTeacherStatus}
									loadingLabel="กำลังโหลด..."
								>
									โหลดสถานะครู
								</LoadingButton>
							</div>
						{/if}
					</div>
					<div
						class="relative space-y-3"
						aria-busy={loadingTeacherStatus}
						data-testid="supervision-teacher-status"
					>
						{#if routeData}{@render managementFeedback(
								teacherStatusLoaded,
								loadingTeacherStatus,
								teacherStatusError,
								loadTeacherStatusOverview
							)}{/if}
						{#if !routeData || teacherStatusLoaded}
							<div data-testid="supervision-teacher-status-ready">
								<Table.Root>
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
													เลือกและโหลดรอบนิเทศเพื่อดูสถานะครู
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
	{:else if section === 'approvals'}
		<div class="space-y-4">
			<Card.Root>
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<BarChart3 class="h-5 w-5" />
						รับรองและอนุมัติผลนิเทศ
					</Card.Title>
					<Card.Description>
						หัวหน้ากลุ่มสาระรับรองผลก่อน แล้วฝ่ายวิชาการอนุมัติผลเพื่อให้ครูรับทราบ
					</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-4">
					<div
						class="relative space-y-3"
						data-testid="supervision-progress"
						aria-busy={loadingProgress}
					>
						{#if canReadSchoolReport && cyclesLoaded}<div class="flex flex-col gap-2 md:flex-row">
								<Select.Root
									type="single"
									value={progressCycleId}
									onValueChange={(id) => {
										progressCycleId = id;
										progressRequest.abort();
										progress = null;
										progressError = '';
										loadingProgress = false;
									}}
								>
									<Select.Trigger class="w-full md:w-[360px]">
										{cycles.find((cycle) => cycle.id === progressCycleId)?.title ?? 'เลือกรอบนิเทศ'}
									</Select.Trigger>
									<Select.Content>
										{#each cycles as cycle (cycle.id)}
											<Select.Item value={cycle.id}>{cycleLabel(cycle)}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
								<LoadingButton
									onclick={loadProgress}
									loading={loadingProgress}
									loadingLabel="กำลังโหลด..."
								>
									โหลดรายงาน
								</LoadingButton>
							</div>
						{/if}
						{@render managementFeedback(
							!!progress,
							loadingProgress,
							progressError,
							() => void loadProgress()
						)}
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
									<p class="text-xl font-semibold">
										{progress.underReviewCount + progress.approvedCount}
									</p>
									<p class="text-xs text-muted-foreground">รอรับรอง/อนุมัติ</p>
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
					</div>
					<div
						class="relative space-y-2"
						data-testid="supervision-observations"
						aria-busy={observationsLoading}
					>
						{@render managementFeedback(
							observationsLoaded,
							observationsLoading,
							observationsError,
							retryObservations
						)}
						{#if observationsLoaded && observationsReadable}
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
									{#if approvalWorkflowObservations.length === 0}
										<Table.Row>
											<Table.Cell colspan={4} class="h-24 text-center text-muted-foreground">
												ยังไม่มีรายการที่ต้องรับรองหรืออนุมัติ
											</Table.Cell>
										</Table.Row>
									{:else}
										{#each approvalWorkflowObservations as observation (observation.id)}
											<Table.Row>
												<Table.Cell>{observation.observedDisplayName ?? 'ครู'}</Table.Cell>
												<Table.Cell>{observationLessonTitle(observation)}</Table.Cell>
												<Table.Cell>
													<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
													<p class="text-xs text-muted-foreground">
														{observationWorkflow(observation).stageLabel} · ถัดไป: {observationWorkflow(
															observation
														).nextOwner}
													</p>
												</Table.Cell>
												<Table.Cell>
													<div class="flex flex-wrap items-center justify-end gap-2">
														<Button
															size="sm"
															variant="outline"
															class="h-8"
															href={observationDetailHref(observation)}
															data-sveltekit-preload-data="tap"
														>
															<Eye class="h-4 w-4" />
															ตรวจผล
														</Button>
													</div>
												</Table.Cell>
											</Table.Row>
										{/each}
									{/if}
								</Table.Body>
							</Table.Root>{/if}
					</div>
				</Card.Content>
			</Card.Root>
		</div>
	{/if}
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

		{@render managementFeedback(
			!!selectedEvaluationTemplate,
			evaluationTemplateLoading,
			evaluationTemplateError,
			() => void loadEvaluationTemplate()
		)}
		{#if selectedEvaluation && selectedEvaluationTemplate && !evaluationTemplateLoading}
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
					onclick={saveEvaluation}
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

<Dialog.Root
	open={createCycleDialogOpen}
	onOpenChange={(open) => {
		if (savingAction !== 'create-cycle') createCycleDialogOpen = open;
	}}
>
	<Dialog.Content
		class="max-w-3xl"
		onEscapeKeydown={(event) => {
			if (savingAction === 'create-cycle') event.preventDefault();
		}}
		onInteractOutside={(event) => {
			if (savingAction === 'create-cycle') event.preventDefault();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>สร้างรอบนิเทศ</Dialog.Title>
			<Dialog.Description>รอบนิเทศจะผูกกับปีและภาคเรียนที่เลือกบนแถบด้านบน</Dialog.Description>
		</Dialog.Header>
		<form
			onsubmit={(event) => {
				event.preventDefault();
				void createCycle();
			}}
		>
			<fieldset
				disabled={savingAction === 'create-cycle'}
				class="grid min-w-0 gap-4 py-2 lg:grid-cols-2"
			>
				<div class="rounded-lg border bg-muted/30 p-3 lg:col-span-2">
					<p class="text-xs font-medium text-muted-foreground">บริบทของรอบนิเทศ</p>
					<p class="mt-1 font-medium">
						{academicYearId ? academicYearLabel(academicYearId) : 'ยังไม่ได้เลือกปีการศึกษา'} ·
						{academicTermLabel(academicTermId)}
					</p>
				</div>
				<div class="space-y-2 lg:col-span-2">
					<Label for="supervision-cycle-title">ชื่อรอบนิเทศ</Label>
					<Input
						id="supervision-cycle-title"
						bind:value={cycleForm.title}
						placeholder="เช่น นิเทศการสอน ภาคเรียนที่ 1"
					/>
				</div>
				<div class="space-y-2 lg:col-span-2">
					<Label for="supervision-cycle-template">แบบประเมิน</Label>
					{#if routeData}{@render managementFeedback(
							templatesLoaded,
							templatesLoading,
							templatesError,
							retryManagementTemplates
						)}{/if}
					{#if !routeData || templatesLoaded}
						<Select.Root type="single" bind:value={cycleForm.templateId}>
							<Select.Trigger id="supervision-cycle-template" class="w-full">
								{templateChoices.find((template) => template.id === cycleForm.templateId)?.title ??
									'เลือกแบบประเมิน'}
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
				<div class="space-y-2 lg:col-span-2">
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
						ครูที่มีสิทธิ์ส่งคำขอได้เมื่อรอบมีสถานะเปิดรับคำขอ ผู้ดูแลเปลี่ยนสถานะได้ด้วยตนเอง
					</p>
				</div>
				<div class="space-y-2 lg:col-span-2">
					<Label for="supervision-cycle-description">รายละเอียด</Label>
					<Textarea
						id="supervision-cycle-description"
						bind:value={cycleForm.description}
						rows={2}
						placeholder="รายละเอียดเพิ่มเติม"
					/>
				</div>
			</fieldset>
			{#if cycleFormError}<p role="alert" class="my-3 text-sm text-destructive">
					{cycleFormError}
				</p>{/if}
			<Dialog.Footer>
				<Button
					type="button"
					variant="outline"
					disabled={savingAction === 'create-cycle'}
					onclick={() => (createCycleDialogOpen = false)}>ยกเลิก</Button
				>
				<LoadingButton
					type="submit"
					disabled={routeData && (!templatesLoaded || templatesLoading || !!templatesError)}
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
		<div
			class="relative min-h-0 flex-1 space-y-4 overflow-y-auto overflow-x-hidden px-4 py-4 sm:px-6"
			aria-busy={templateDetailLoading}
		>
			{#if routeData}{@render managementFeedback(
					templateDetailLoaded,
					templateDetailLoading,
					templateDetailError,
					() => {
						void loadManagementTemplateDetail('edit', editingTemplateId);
					}
				)}{/if}
			{#if !routeData || templateDetailLoaded}
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
											<div
												class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
											>
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
			{/if}
		</div>
		<Dialog.Footer class="border-t px-4 py-4 sm:px-6">
			<Button variant="outline" onclick={() => (createTemplateDialogOpen = false)}>ยกเลิก</Button>
			<LoadingButton
				onclick={createTemplate}
				disabled={routeData &&
					(!templateDetailLoaded || templateDetailLoading || !!templateDetailError)}
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
