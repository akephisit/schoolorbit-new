<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { page } from '$app/state';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import SupervisionLessonPicker from '#lib/components/supervision/SupervisionLessonPicker.svelte';
	import SupervisionSavedPeriods from '#lib/components/supervision/SupervisionSavedPeriods.svelte';
	import SupervisionRequestReview from '#lib/components/supervision/SupervisionRequestReview.svelte';
	import { createSupervisionRequestReview } from '#lib/supervision/request-review.svelte.js';
	import {
		schoolCivilDate,
		cycleDateRangeError,
		cycleDateRangeLabel,
		type SupervisionLessonSelection
	} from '#lib/supervision/booking-range.js';
	import {
		savedLessonLabels,
		savedPeriodSourceLabel
	} from '#lib/supervision/supervision-presentation.js';
	import {
		supervisionStatusLabel as statusLabel,
		currentReturnReason
	} from '#lib/supervision/supervision-presentation.js';
	import SupervisionWorkflow from '#lib/components/supervision/SupervisionWorkflow.svelte';
	import { isAbortError, LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { academicContextualMenuPath } from '#lib/academic-context/route-context.js';
	import {
		resolveSupervisionRead,
		waitForSupervisionAccess,
		SUPERVISION_OBSERVATION_READ_PERMISSIONS
	} from '#lib/supervision/supervision-access.js';
	import {
		ArrowLeft,
		CalendarClock,
		Check,
		ChevronsUpDown,
		RefreshCw,
		Trash2,
		UserCheck
	} from '@lucide/svelte';
	import { toast } from 'svelte-sonner';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import {
		approveSupervisionObservation,
		cancelRequestedSupervisionObservation,
		cancelSupervisionObservation,
		certifySupervisionObservation,
		getSupervisionEvaluatorAvailability,
		getSupervisionObservation,
		getSupervisionObservationReview,
		getSupervisionTemplateSummary,
		getSupervisionCycle,
		replaceSupervisionObservationEvaluators,
		resubmitSupervisionObservationRequest,
		updateRequestedSupervisionObservation,
		updateSupervisionObservation,
		type SupervisionCycle,
		type SupervisionEvaluatorAvailability,
		type SupervisionEvaluator,
		type SupervisionObservation,
		type SupervisionObservationReview,
		type SupervisionObservationStatus,
		type SupervisionReviewEvaluatorResult,
		type SupervisionReviewResponse,
		type SupervisionTemplate,
		type SupervisionTemplateSummary
	} from '#lib/api/supervision.js';
	import {
		calculateRubricDraftSummary,
		qualityLevelFromPercentage,
		sectionRubricProgress,
		type RubricFormSection,
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
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import type { PageData } from './$types';

	type DetailItem = {
		label: string;
		value: string;
	};

	let { data }: { data: PageData } = $props();
	const academicContext = getAcademicContextStore();
	const academicContextOptions = $derived($academicContext.options);

	let loading = $state(true);
	let savingAction = $state<string | null>(null);
	let error = $state('');
	let resubmitError = $state('');
	let observation = $state<SupervisionObservation | null>(null);
	let review = $state<SupervisionObservationReview | null>(null);
	let template = $state<SupervisionTemplateSummary | null>(null);
	let cycle = $state<SupervisionCycle | null>(null);
	let cycleLoading = $state(true);
	let cycleError = $state('');
	let templateLoading = $state(true);
	let templateError = $state('');
	let reviewOpen = $state(false);
	let evaluatorAvailabilityError = $state('');
	let evaluatorAvailabilityLoaded = $state(false);
	let contextEpoch = 0;
	let contextId = '';
	let contextIdentity = '';
	let mutationLifetime = 0;
	onDestroy(() => {
		mutationLifetime += 1;
		requestReview.reset();
	});

	function captureMutationOwner() {
		const lifetime = mutationLifetime;
		const epoch = contextEpoch;
		const identity = appIdentityKey();
		const resourceId = data.observationId;
		const pathname = page.url.pathname;
		const year = data.academicYearId;
		const term = data.academicTermId;
		return () =>
			lifetime === mutationLifetime &&
			epoch === contextEpoch &&
			identity === appIdentityKey() &&
			resourceId === data.observationId &&
			year === data.academicYearId &&
			term === data.academicTermId &&
			pathname === page.url.pathname;
	}
	let lessonWorkflowRevision = 0;
	let evaluatorWorkflowRevision = 0;
	const observationRequest = new LatestRequest();
	const cycleRequest = new LatestRequest();
	const templateRequest = new LatestRequest();
	const reviewRequest = new LatestRequest();
	const evaluatorRequest = new LatestRequest();
	let availableEvaluators = $state<SupervisionEvaluatorAvailability[]>([]);
	let loadingReview = $state(false);
	let reviewError = $state('');
	let selectedReviewEvaluatorId = $state('summary');
	let loadingEvaluatorAvailability = $state(false);
	let editLessonOpen = $state(false);
	let editEvaluatorsOpen = $state(false);
	let requestReviewOpen = $state(false);
	let evaluatorPickerOpen = $state(false);
	let cancelDialogOpen = $state(false);
	let cancelReason = $state('');
	let selectedEvaluatorIds = $state<string[]>([]);

	const currentUserId = $derived($authStore.user?.id ?? '');
	const canManageObservation = $derived(
		observation?.canManage === true &&
			$can.hasAny(
				PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_UNIT,
				PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_TREE,
				PERMISSIONS.SUPERVISION_MANAGE_SCHOOL
			)
	);
	const canApproveSchool = $derived($can.has(PERMISSIONS.SUPERVISION_APPROVE_SCHOOL));
	const canViewReviewDetails = $derived(
		Boolean(
			observation?.canReview === true && $can.hasAny(...SUPERVISION_OBSERVATION_READ_PERMISSIONS)
		)
	);
	const canCertifyResult = $derived(
		Boolean(
			observation &&
			canManageObservation &&
			(observation.status === 'evaluators_submitted' || observation.status === 'under_review')
		)
	);
	const canApproveResult = $derived(
		Boolean(observation && canApproveSchool && observation.status === 'approved')
	);
	const canCancelOwnRequest = $derived(
		Boolean(
			observation &&
			observation.observedUserId === currentUserId &&
			(observation.status === 'requested' || observation.status === 'returned') &&
			$can.has(PERMISSIONS.SUPERVISION_REQUEST_OWN)
		)
	);
	const canEditOwnRequested = $derived(
		canCancelOwnRequest &&
			!observation?.evaluators.some((evaluator) => evaluator.status === 'submitted')
	);
	const isReturnedOwnRequest = $derived(canCancelOwnRequest && observation?.status === 'returned');
	const canResubmitSaved = $derived(
		canEditOwnRequested && isReturnedOwnRequest && observation?.periodsLocked === true
	);
	const cycleDatesError = $derived(
		cycle ? cycleDateRangeError(cycle.observationStartDate, cycle.observationEndDate) : null
	);
	const cycleAllowsResubmission = $derived(
		!cycleLoading && cycle?.status === 'open' && !cycleDatesError
	);
	const canEditEvaluators = $derived(
		Boolean(
			observation &&
			(observation.status === 'planned' || observation.status === 'returned') &&
			canManageObservation
		)
	);
	const requestReview = createSupervisionRequestReview({
		getObservation: (id) => (observation?.id === id ? observation : null),
		hasManagePermission: () => canManageObservation,
		captureOwner: captureMutationOwner,
		isBusy: () => savingAction !== null,
		getSavingAction: () => savingAction,
		setSavingAction: (action) => (savingAction = action),
		onApplied: (updated) => {
			replaceObservation(updated);
			setRequestReviewOpen(false);
		},
		onDenied: () => {
			requestReviewOpen = false;
			observationRequest.abort();
			void retryObservation();
		},
		allowSaveEvaluators: true
	});
	const canEditLesson = $derived(
		Boolean(
			observation &&
			!requestReview.isDenied(observation.id) &&
			!observation.periodsLocked &&
			manageableStatus(observation.status) &&
			(canManageObservation || canEditOwnRequested)
		)
	);
	const canReviewPendingRequest = $derived(
		Boolean(observation && requestReview.canReview(observation))
	);
	const canCancelObservation = $derived(
		Boolean(
			observation &&
			!requestReview.isDenied(observation.id) &&
			(canCancelOwnRequest ||
				(canManageObservation &&
					!['published', 'acknowledged', 'completed', 'cancelled'].includes(observation.status)))
		)
	);
	const backHref = $derived(
		academicContextualMenuPath(
			'/staff/academic/supervision',
			{
				academicYearId: observation?.academicYearId ?? data.academicYearId,
				academicTermId: observation?.academicTermId ?? data.academicTermId
			},
			academicContextOptions
		)
	);
	const pageTitle = $derived(observation?.observedDisplayName ?? 'รายละเอียดรายการนิเทศ');
	const selectedEvaluators = $derived(
		availableEvaluators.filter((staff) => selectedEvaluatorIds.includes(staff.id))
	);
	const unavailableEvaluatorCount = $derived(
		availableEvaluators.filter((evaluator) => !evaluator.available).length
	);
	const reviewRubricSections = $derived(
		review ? templateSectionsToRubricForm(review.template) : []
	);
	const selectedReviewEvaluator = $derived(
		review?.evaluatorResults.find((result) => result.evaluatorId === selectedReviewEvaluatorId) ??
			null
	);
	const selectedReviewDrafts = $derived(
		review
			? selectedReviewEvaluatorId === 'summary'
				? reviewSummaryDrafts(review)
				: selectedReviewEvaluator
					? reviewEvaluatorDrafts(selectedReviewEvaluator)
					: {}
			: {}
	);
	const selectedReviewSummary = $derived(
		review
			? calculateRubricDraftSummary(
					reviewRubricSections,
					selectedReviewDrafts,
					review.template.ratingMax
				)
			: null
	);

	const routeObservationId = $derived(data.observationId);
	const routeObservation = $derived(data.observation);
	const routeCycle = $derived(data.cycle);
	const routeTemplate = $derived(data.template);
	const routeIdentity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});

	$effect.pre(() => {
		const source = {
			identity: data.identity,
			observationId: routeObservationId,
			observation: routeObservation,
			cycle: routeCycle,
			template: routeTemplate
		};
		const identity = routeIdentity;
		const context = [source.observationId, data.academicYearId, data.academicTermId].join(':');
		untrack(() => {
			if (contextId !== context || contextIdentity !== identity) {
				contextIdentity = identity;
				resubmitError = '';
				contextId = context;
				contextEpoch += 1;
				observation = null;
				cycle = null;
				template = null;
				review = null;
				savingAction = null;
				reviewOpen = false;
				editLessonOpen = false;
				editEvaluatorsOpen = false;
				requestReviewOpen = false;
				requestReview.reset();
				evaluatorPickerOpen = false;
				cancelDialogOpen = false;
				cancelReason = '';
				selectedReviewEvaluatorId = 'summary';
				selectedEvaluatorIds = [];
				availableEvaluators = [];
				evaluatorAvailabilityLoaded = false;
				reviewError = '';
				evaluatorAvailabilityError = '';
				loadingReview = false;
				loadingEvaluatorAvailability = false;
				reviewRequest.abort();
				evaluatorRequest.abort();
			}
			const identityTicket = observationRequest.begin();
			const cycleTicket = cycleRequest.begin();
			const templateTicket = templateRequest.begin();
			loading = true;
			error = '';
			cycleLoading = true;
			cycleError = '';
			templateLoading = true;
			templateError = '';
			void Promise.all([source.identity, source.observation]).then(([owner, result]) => {
				if (
					owner !== appIdentityKey() ||
					identity !== appIdentityKey() ||
					!observationRequest.isCurrent(identityTicket.revision)
				)
					return;
				loading = false;
				if (result.ok) applyObservation(result.data);
				else error = result.error;
			});
			void Promise.all([source.identity, source.cycle]).then(([owner, result]) => {
				if (
					owner !== appIdentityKey() ||
					identity !== appIdentityKey() ||
					!cycleRequest.isCurrent(cycleTicket.revision)
				)
					return;
				cycleLoading = false;
				if (result.ok) cycle = result.data;
				else cycleError = result.error;
			});
			void Promise.all([source.identity, source.template]).then(([owner, result]) => {
				if (
					owner !== appIdentityKey() ||
					identity !== appIdentityKey() ||
					!templateRequest.isCurrent(templateTicket.revision)
				)
					return;
				templateLoading = false;
				if (result.ok) template = result.data;
				else templateError = result.error;
			});
		});
		return () => {
			observationRequest.abort();
			cycleRequest.abort();
			templateRequest.abort();
			reviewRequest.abort();
			evaluatorRequest.abort();
		};
	});

	$effect(() => {
		if (!canViewReviewDetails) {
			reviewOpen = false;
			review = null;
		}
		if (!canEditLesson) editLessonOpen = false;
		if (!canEditEvaluators) {
			editEvaluatorsOpen = false;
			availableEvaluators = [];
		}
		if (!canReviewPendingRequest) requestReviewOpen = false;
		if (!canCancelObservation) cancelDialogOpen = false;
		if (!reviewOpen) {
			reviewRequest.abort();
			loadingReview = false;
		}
		if (!editEvaluatorsOpen) {
			evaluatorRequest.abort();
			loadingEvaluatorAvailability = false;
		}
	});

	function manageableStatus(status: SupervisionObservationStatus): boolean {
		return status === 'requested' || status === 'planned' || status === 'returned';
	}

	function observationResultsReleased(status: SupervisionObservationStatus): boolean {
		return status === 'published' || status === 'acknowledged' || status === 'completed';
	}

	function observationAverageScoreLabel(item: SupervisionObservation): string {
		if (!item.canReview && !observationResultsReleased(item.status))
			return 'รอหัวหน้ากลุ่มบริหารวิชาการอนุมัติผล';
		if (item.averageRating !== null && item.averageRating !== undefined) {
			return item.averageRating.toFixed(2);
		}
		if (!observationResultsReleased(item.status)) {
			return 'รอหัวหน้ากลุ่มบริหารวิชาการอนุมัติผล';
		}
		return '-';
	}

	function reviewableStatus(status: SupervisionObservationStatus): boolean {
		return [
			'evaluators_submitted',
			'under_review',
			'approved',
			'published',
			'acknowledged',
			'completed'
		].includes(status);
	}

	function templateSectionsToRubricForm(source: SupervisionTemplate): RubricFormSection[] {
		return source.sections.map((section) => ({
			localId: section.id,
			title: section.title,
			description: section.description ?? '',
			sortOrder: section.sortOrder,
			items: section.items.map((item) => ({
				localId: item.id,
				label: item.label,
				description: item.description ?? '',
				itemType: item.itemType,
				required: item.required,
				sortOrder: item.sortOrder
			}))
		}));
	}

	function ratingScale(min: number, max: number): number[] {
		const start = Math.min(min, max);
		const end = Math.max(min, max);
		return Array.from({ length: end - start + 1 }, (_, index) => end - index);
	}

	function reviewAverageScoreLabel(score?: number | null): string {
		return score === null || score === undefined ? '-' : score.toFixed(2);
	}

	function reviewResponseFor(
		responses: SupervisionReviewResponse[],
		templateItemId: string
	): SupervisionReviewResponse | null {
		return responses.find((response) => response.templateItemId === templateItemId) ?? null;
	}

	function reviewItemSummaryFor(templateItemId: string) {
		return (
			review?.itemSummaries.find((summary) => summary.templateItemId === templateItemId) ?? null
		);
	}

	function reviewItemRatingFor(item: { localId: string }): number | null {
		if (selectedReviewEvaluator) {
			return (
				reviewResponseFor(selectedReviewEvaluator.responses, item.localId)?.ratingScore ?? null
			);
		}
		return reviewItemSummaryFor(item.localId)?.averageRating ?? null;
	}

	function reviewItemTextFor(item: { localId: string }): string {
		if (selectedReviewEvaluator) {
			return reviewResponseFor(selectedReviewEvaluator.responses, item.localId)?.textResponse ?? '';
		}
		return (
			review?.evaluatorResults
				.map((result) => reviewResponseFor(result.responses, item.localId)?.textResponse?.trim())
				.filter(Boolean)
				.join('\n\n') ?? ''
		);
	}

	function reviewEvaluatorDrafts(
		evaluator: SupervisionReviewEvaluatorResult
	): Record<string, RubricResponseDraft> {
		return Object.fromEntries(
			evaluator.responses.map((response) => [
				response.templateItemId,
				{
					ratingScore:
						response.ratingScore === null || response.ratingScore === undefined
							? ''
							: String(response.ratingScore),
					textResponse: response.textResponse ?? ''
				}
			])
		);
	}

	function reviewSummaryDrafts(
		source: SupervisionObservationReview
	): Record<string, RubricResponseDraft> {
		return Object.fromEntries(
			source.itemSummaries.map((summary) => [
				summary.templateItemId,
				{
					ratingScore:
						summary.averageRating === null || summary.averageRating === undefined
							? ''
							: String(summary.averageRating),
					textResponse: ''
				}
			])
		);
	}

	function evaluatorStatusLabel(status: SupervisionEvaluator['status']): string {
		return (
			{
				assigned: 'มอบหมายแล้ว',
				draft: 'ยังไม่ส่ง',
				submitted: 'ส่งผลแล้ว'
			}[status] ?? status
		);
	}

	function formatDateTime(value?: string | null): string {
		if (!value) return '-';
		return new Date(value).toLocaleString('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short'
		});
	}

	function observationSubjectLabel(item: SupervisionObservation): string {
		return savedLessonLabels(item).subject;
	}
	function observationClassroomLabel(item: SupervisionObservation): string {
		return savedLessonLabels(item).homeroom;
	}
	function observationRoomLabel(item: SupervisionObservation): string {
		return savedLessonLabels(item).room;
	}
	function observationPeriodLabel(item: SupervisionObservation): string {
		return savedLessonLabels(item).period;
	}
	function hasDetachedTimetableSource(item: SupervisionObservation): boolean {
		return item.periods.some(
			(period) =>
				!period.timetableBlockGroupId &&
				!period.manualLesson &&
				(period.lessonSnapshot.source === 'timetable' ||
					period.lessonSnapshot.source === 'removed_timetable' ||
					Boolean(period.lessonSnapshot.timetableBlockGroupId))
		);
	}
	function lessonSourceLabel(item: SupervisionObservation): string {
		return [...new Set(item.periods.map(savedPeriodSourceLabel))].join(' · ');
	}

	function lessonDetails(item: SupervisionObservation): DetailItem[] {
		return [
			{ label: 'วิชา', value: observationSubjectLabel(item) },
			{ label: 'คาบ', value: observationPeriodLabel(item) },
			{ label: 'ชั้น/ห้อง', value: observationClassroomLabel(item) },
			{ label: 'ห้องเรียน', value: observationRoomLabel(item) },
			{ label: 'วันที่/เวลา', value: formatDateTime(item.observedAt) },
			{ label: 'ที่มา', value: lessonSourceLabel(item) }
		];
	}

	function mutationData<T>(
		response: { success: boolean; data?: T; error?: string },
		fallback: string
	): T {
		if (!response.success || !response.data) {
			throw new Error(response.error || fallback);
		}
		return response.data;
	}

	function applyObservation(updated: SupervisionObservation | null, freshRead = true) {
		observation = updated;
		requestReview.reconcile(freshRead);
		if (!updated || !requestReview.canReview(updated)) requestReviewOpen = false;
		if (!updated) {
			contextEpoch += 1;
			savingAction = null;
			reviewRequest.abort();
			cycleRequest.abort();
			templateRequest.abort();
			review = null;
			cycle = null;
			template = null;
			loadingReview = false;
			cycleLoading = false;
			templateLoading = false;
		}
	}

	function replaceObservation(updated: SupervisionObservation) {
		observationRequest.abort();
		reviewRequest.abort();
		loading = false;
		loadingReview = false;
		applyObservation(updated, false);
		if (review)
			review = {
				...review,
				observation: updated,
				averageRating: updated.averageRating ?? review.averageRating
			};
	}

	async function retryObservation() {
		if (!(await waitForSupervisionAccess(SUPERVISION_OBSERVATION_READ_PERMISSIONS))) return;
		const ticket = observationRequest.begin();
		const owner = appIdentityKey();
		loading = true;
		error = '';
		const result = await captureRouteLoad(
			resolveSupervisionRead(
				getSupervisionObservation(data.observationId, { signal: ticket.signal })
			),
			'โหลดรายการนิเทศไม่สำเร็จ'
		);
		if (owner !== appIdentityKey() || !observationRequest.isCurrent(ticket.revision)) return;
		loading = false;
		if (result.ok) {
			applyObservation(result.data);
			if (result.data && !cycle && !cycleLoading) void retryCycle();
			if (result.data && !template && !templateLoading) void retryTemplate();
		} else error = result.error;
	}

	async function retryCycle() {
		if (!observation) return;
		const ticket = cycleRequest.begin();
		const owner = appIdentityKey();
		cycleLoading = true;
		cycleError = '';
		const result = await captureRouteLoad(
			getSupervisionCycle(observation.cycleId, { signal: ticket.signal }),
			'โหลดรอบนิเทศไม่สำเร็จ'
		);
		if (owner !== appIdentityKey() || !cycleRequest.isCurrent(ticket.revision)) return;
		cycleLoading = false;
		if (result.ok) cycle = result.data;
		else cycleError = result.error;
	}

	async function retryTemplate() {
		if (!observation) return;
		const ticket = templateRequest.begin();
		const owner = appIdentityKey();
		templateLoading = true;
		templateError = '';
		const result = await captureRouteLoad(
			getSupervisionTemplateSummary(observation.templateId, { signal: ticket.signal }),
			'โหลดข้อมูลแบบประเมินไม่สำเร็จ'
		);
		if (owner !== appIdentityKey() || !templateRequest.isCurrent(ticket.revision)) return;
		templateLoading = false;
		if (result.ok) template = result.data;
		else templateError = result.error;
	}

	function observationContextLabel(item: SupervisionObservation): string {
		const year = academicContextOptions?.years.find((option) => option.id === item.academicYearId);
		const term = academicContextOptions?.terms.find((option) => option.id === item.academicTermId);
		return `${term?.name ?? 'ไม่พบภาคเรียน'} / ${year?.name ?? 'ไม่พบปีการศึกษา'}`;
	}

	async function loadReviewDetail(id = observation?.id) {
		if (
			!id ||
			!canViewReviewDetails ||
			!reviewOpen ||
			!observation ||
			!reviewableStatus(observation.status)
		)
			return;
		const ticket = reviewRequest.begin();
		loadingReview = true;
		reviewError = '';
		try {
			const loadedReview = await getSupervisionObservationReview(id, { signal: ticket.signal });
			if (!reviewRequest.isCurrent(ticket.revision) || !reviewOpen) return;
			review = loadedReview;
			if (
				selectedReviewEvaluatorId !== 'summary' &&
				!loadedReview.evaluatorResults.some(
					(result) => result.evaluatorId === selectedReviewEvaluatorId
				)
			)
				selectedReviewEvaluatorId = 'summary';
		} catch (loadError) {
			if (!reviewRequest.isCurrent(ticket.revision) || isAbortError(loadError)) return;
			reviewError =
				loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดผลประเมินนิเทศได้';
		} finally {
			if (reviewRequest.isCurrent(ticket.revision)) loadingReview = false;
		}
	}

	function openReview() {
		reviewOpen = true;
		void loadReviewDetail();
	}

	function replaceReviewObservation(updated: SupervisionObservation) {
		replaceObservation(updated);
	}

	async function certifyResult() {
		if (!observation || !canCertifyResult) return;
		const ownsMutation = captureMutationOwner();
		const submittedId = observation.id;
		savingAction = `certify-result:${observation.id}`;
		try {
			const response = await certifySupervisionObservation(submittedId);
			if (!ownsMutation()) return;
			const updated = mutationData(response, 'รับรองผลไม่สำเร็จ');
			replaceReviewObservation(updated);
			toast.success('รับรองผลนิเทศแล้ว');
		} catch (saveError) {
			if (!ownsMutation()) return;
			toast.error(saveError instanceof Error ? saveError.message : 'รับรองผลไม่สำเร็จ');
		} finally {
			if (ownsMutation()) savingAction = null;
		}
	}

	async function approveResult() {
		if (!observation || !canApproveResult) return;
		const ownsMutation = captureMutationOwner();
		const submittedId = observation.id;
		savingAction = `approve-result:${observation.id}`;
		try {
			const response = await approveSupervisionObservation(submittedId);
			if (!ownsMutation()) return;
			const updated = mutationData(response, 'อนุมัติผลไม่สำเร็จ');
			replaceReviewObservation(updated);
			toast.success('อนุมัติผลนิเทศแล้ว');
		} catch (saveError) {
			if (!ownsMutation()) return;
			toast.error(saveError instanceof Error ? saveError.message : 'อนุมัติผลไม่สำเร็จ');
		} finally {
			if (ownsMutation()) savingAction = null;
		}
	}

	function openLessonEditor() {
		if (!observation || !canEditLesson || savingAction) return;
		lessonWorkflowRevision += 1;
		editLessonOpen = true;
	}

	async function saveLessonEdit(selection: SupervisionLessonSelection) {
		if (!observation || !canEditLesson) throw new Error('ไม่มีสิทธิ์แก้ไขคาบนิเทศ');
		if (savingAction) throw new Error('รอให้การบันทึกปัจจุบันเสร็จก่อน');
		if (isReturnedOwnRequest && cycle?.status !== 'open')
			throw new Error('รอบนิเทศยังไม่เปิดรับคำขอ');
		const ownsMutation = captureMutationOwner();
		const submittedId = observation.id;
		const workflowRevision = lessonWorkflowRevision;
		const resubmitting = isReturnedOwnRequest;
		savingAction = 'lesson';
		try {
			const response =
				canManageObservation && !canEditOwnRequested
					? await updateSupervisionObservation(submittedId, selection)
					: await updateRequestedSupervisionObservation(submittedId, selection);
			if (!ownsMutation() || !canEditLesson) return;
			replaceObservation(mutationData(response, 'แก้ไขรายการนิเทศไม่สำเร็จ'));
			if (workflowRevision === lessonWorkflowRevision) editLessonOpen = false;
			toast.success(resubmitting ? 'แก้ไขและส่งคำขอใหม่แล้ว' : 'แก้ไขคาบนิเทศแล้ว');
		} catch (saveError) {
			if (ownsMutation()) throw saveError;
		} finally {
			if (ownsMutation()) savingAction = null;
		}
	}

	function setRequestReviewOpen(open: boolean) {
		if (!observation) return;
		if (open && (!requestReview.canReview(observation) || savingAction)) return;
		requestReviewOpen = open;
		if (open) void requestReview.loadAvailability(observation.id);
		else requestReview.cancelAvailability(observation.id);
	}

	async function openEvaluatorEditor() {
		if (!observation || !canEditEvaluators || savingAction) return;
		evaluatorWorkflowRevision += 1;
		selectedEvaluatorIds = observation.evaluators.map((evaluator) => evaluator.evaluatorUserId);
		editEvaluatorsOpen = true;
		void loadEvaluatorAvailability(true);
	}

	async function loadEvaluatorAvailability(force = false) {
		if (!observation || !canEditEvaluators || !editEvaluatorsOpen) return;
		if (!force && evaluatorAvailabilityLoaded) return;
		const ticket = evaluatorRequest.begin();
		loadingEvaluatorAvailability = true;
		evaluatorAvailabilityError = '';
		try {
			const result = await getSupervisionEvaluatorAvailability(observation.id, {
				signal: ticket.signal
			});
			if (!evaluatorRequest.isCurrent(ticket.revision) || !editEvaluatorsOpen) return;
			const items = result.items;
			availableEvaluators = items;
			evaluatorAvailabilityLoaded = true;
			const availableIds = new Set(items.filter((item) => item.available).map((item) => item.id));
			selectedEvaluatorIds = selectedEvaluatorIds.filter((id) => availableIds.has(id));
		} catch (loadError) {
			if (!evaluatorRequest.isCurrent(ticket.revision) || isAbortError(loadError)) return;
			evaluatorAvailabilityError =
				loadError instanceof Error ? loadError.message : 'ไม่สามารถตรวจสอบผู้ประเมินที่ว่างได้';
		} finally {
			if (evaluatorRequest.isCurrent(ticket.revision)) loadingEvaluatorAvailability = false;
		}
	}

	function toggleEvaluator(staff: SupervisionEvaluatorAvailability) {
		if (!canEditEvaluators) return;
		if (!staff.available) return;
		selectedEvaluatorIds = selectedEvaluatorIds.includes(staff.id)
			? selectedEvaluatorIds.filter((id) => id !== staff.id)
			: [...selectedEvaluatorIds, staff.id];
	}

	function removeEvaluator(id: string) {
		selectedEvaluatorIds = selectedEvaluatorIds.filter((item) => item !== id);
	}

	async function saveEvaluatorEdit() {
		if (!observation || !canEditEvaluators || savingAction) return;
		if (selectedEvaluatorIds.length === 0) {
			toast.error('เลือกผู้ประเมินอย่างน้อย 1 คน');
			return;
		}
		const ownsMutation = captureMutationOwner();
		const submittedId = observation.id;
		const workflowRevision = evaluatorWorkflowRevision;
		savingAction = 'evaluators';
		try {
			const response = await replaceSupervisionObservationEvaluators(submittedId, {
				evaluators: selectedEvaluatorIds.map((evaluatorUserId) => ({
					evaluatorUserId,
					isRequired: true
				}))
			});
			if (!ownsMutation()) return;
			const updated = mutationData(response, 'แก้ไขผู้ประเมินไม่สำเร็จ');
			replaceObservation(updated);
			if (workflowRevision === evaluatorWorkflowRevision) editEvaluatorsOpen = false;
			toast.success('แก้ไขผู้ประเมินแล้ว');
		} catch (saveError) {
			if (!ownsMutation()) return;
			void loadEvaluatorAvailability(true);
			if (!ownsMutation()) return;
			toast.error(saveError instanceof Error ? saveError.message : 'แก้ไขผู้ประเมินไม่สำเร็จ');
		} finally {
			if (ownsMutation()) savingAction = null;
		}
	}

	async function resubmitSaved() {
		if (!observation || !canResubmitSaved || !cycleAllowsResubmission || savingAction) return;
		const ownsMutation = captureMutationOwner();
		const submittedId = observation.id;
		savingAction = 'resubmit-saved';
		resubmitError = '';
		try {
			const response = await resubmitSupervisionObservationRequest(submittedId);
			if (!ownsMutation()) return;
			replaceObservation(mutationData(response, 'ส่งคำขอเดิมใหม่ไม่สำเร็จ'));
			toast.success('ส่งคำขอใหม่โดยใช้ช่วงคาบเดิมแล้ว');
		} catch (failure) {
			if (ownsMutation())
				resubmitError = failure instanceof Error ? failure.message : 'ส่งคำขอเดิมใหม่ไม่สำเร็จ';
		} finally {
			if (ownsMutation()) savingAction = null;
		}
	}

	async function cancelObservation() {
		if (!observation || !canCancelObservation || savingAction) return;
		const ownsMutation = captureMutationOwner();
		const submittedId = observation.id;
		savingAction = 'cancel';
		try {
			const response =
				canManageObservation && !canCancelOwnRequest
					? await cancelSupervisionObservation(submittedId, { reason: cancelReason || null })
					: await cancelRequestedSupervisionObservation(submittedId);
			if (!ownsMutation()) return;
			const updated = mutationData(response, 'ยกเลิกรายการนิเทศไม่สำเร็จ');
			replaceObservation(updated);
			cancelDialogOpen = false;
			cancelReason = '';
			toast.success('ยกเลิกรายการนิเทศแล้ว');
		} catch (cancelError) {
			if (!ownsMutation()) return;
			toast.error(
				cancelError instanceof Error ? cancelError.message : 'ยกเลิกรายการนิเทศไม่สำเร็จ'
			);
		} finally {
			if (ownsMutation()) savingAction = null;
		}
	}
</script>

<PageShell
	title={pageTitle}
	description="รายละเอียดคาบนิเทศ ผู้ประเมิน ผลประเมิน และการจัดการรายการเดียว"
	{backHref}
	backLabel="กลับหน้านิเทศการสอน"
>
	{#snippet actions()}
		<Button variant="outline" href={backHref} data-sveltekit-preload-data={false}>
			<ArrowLeft class="h-4 w-4" />
			กลับ
		</Button>
	{/snippet}

	<div class="space-y-4" data-testid="supervision-detail" aria-busy={loading}>
		{#if loading && !observation}
			<div role="status" aria-label="กำลังโหลด"><PageSkeleton /></div>
		{:else if !observation && error}
			<PageState title="โหลดรายการนิเทศไม่สำเร็จ" description={error} />
			<Button variant="outline" onclick={retryObservation}>ลองใหม่</Button>
		{:else if !observation}
			<PageState
				title="ไม่พบรายการนิเทศ"
				description="รายการนี้อาจถูกลบหรือคุณไม่มีสิทธิ์เข้าถึง"
			/>
		{:else}
			{#if error}<PageState title="อัปเดตรายการไม่สำเร็จ" description={error} /><Button
					variant="outline"
					onclick={retryObservation}>ลองใหม่</Button
				>{/if}
			<div class="space-y-3">
				<SupervisionWorkflow {observation} />
				{#if resubmitError}<div
						role="alert"
						class="space-y-2 rounded-lg border border-destructive/20 p-3 text-sm text-destructive"
					>
						<p>{resubmitError}</p>
						<Button
							size="sm"
							variant="outline"
							onclick={() => {
								resubmitError = '';
								void retryObservation();
							}}>ตรวจรายการล่าสุด</Button
						>
					</div>{/if}
				{#if observation.status === 'returned'}
					<div
						class="rounded-lg border bg-muted/30 p-4"
						data-testid="supervision-current-return-reason"
					>
						<p class="text-sm font-medium">เหตุผลที่ส่งกลับ</p>
						<p class="mt-1 whitespace-pre-wrap text-sm">
							{currentReturnReason(observation) ?? 'ไม่ได้ระบุเหตุผล'}
						</p>
						{#if isReturnedOwnRequest && canEditOwnRequested && cycle && cycle.status !== 'open'}<p
								class="mt-2 text-sm text-muted-foreground"
							>
								ส่งใหม่ได้เมื่อผู้ดูแลเปิดรับคำขอในรอบนี้
							</p>{:else if isReturnedOwnRequest && canEditOwnRequested && cycleDatesError}<p
								class="mt-2 text-sm text-muted-foreground"
							>
								ส่งใหม่ได้เมื่อผู้ดูแลกำหนดช่วงวันที่นิเทศของรอบนี้
							</p>{/if}
					</div>
				{/if}
			</div>
			<div class="grid min-w-0 gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
				<div class="min-w-0 space-y-4">
					<Card.Root>
						<Card.Header>
							<div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
								<div class="min-w-0 space-y-1">
									<Card.Title class="flex flex-wrap items-center gap-2">
										{observationSubjectLabel(observation)}
										<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
									</Card.Title>
									<Card.Description>
										{observationPeriodLabel(observation)} · {observationClassroomLabel(observation)}
									</Card.Description>
									{#if hasDetachedTimetableSource(observation)}<p
											class="text-sm text-muted-foreground"
										>
											คาบเดิมถูกนำออกจากตารางปัจจุบัน ข้อมูลคาบและผลการนิเทศที่บันทึกไว้ยังคงอยู่
										</p>{/if}
								</div>
								<div class="flex flex-wrap gap-2">
									{#if canResubmitSaved}<LoadingButton
											variant="outline"
											onclick={resubmitSaved}
											loading={savingAction === 'resubmit-saved'}
											loadingLabel="กำลังส่งใหม่..."
											disabled={!cycleAllowsResubmission || savingAction !== null}
											>ส่งใหม่ด้วยคาบเดิม</LoadingButton
										>{/if}
									{#if canEditLesson}
										<Button variant="outline" onclick={() => void openLessonEditor()}>
											<CalendarClock class="h-4 w-4" />
											{isReturnedOwnRequest ? 'แก้ไขและส่งใหม่' : 'แก้คาบ/วันเวลา'}
										</Button>
									{/if}
									{#if canReviewPendingRequest}
										<Button
											onclick={() => setRequestReviewOpen(true)}
											disabled={savingAction !== null}
										>
											<UserCheck class="h-4 w-4" />
											ตรวจคำขอ
										</Button>
									{:else if canEditEvaluators}
										<Button variant="outline" onclick={openEvaluatorEditor}>
											<UserCheck class="h-4 w-4" />
											แก้ผู้ประเมิน
										</Button>
									{/if}
									{#if canCancelObservation}
										<Button variant="destructive" onclick={() => (cancelDialogOpen = true)}>
											<Trash2 class="h-4 w-4" />
											ยกเลิก
										</Button>
									{/if}
								</div>
							</div>
						</Card.Header>
						<Card.Content class="space-y-4">
							<div class="grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-3">
								{#each lessonDetails(observation) as detail (detail.label)}
									<div class="rounded-md border bg-muted/20 p-3">
										<p class="text-xs text-muted-foreground">{detail.label}</p>
										<p class="font-medium">{detail.value}</p>
									</div>
								{/each}
							</div>
							<SupervisionSavedPeriods {observation} />
							{#if observation.periodsLocked}<p
									class="mt-3 rounded-lg border bg-muted/30 p-3 text-sm"
									data-testid="supervision-periods-locked"
								>
									คาบนิเทศได้รับการอนุมัติแล้ว จึงเปลี่ยนวิชา ห้อง วันและช่วงคาบไม่ได้
									ข้อมูลผู้ประเมินและการจัดการอื่นใช้สิทธิ์ของแต่ละรายการตามเดิม
								</p>{/if}
						</Card.Content>
					</Card.Root>

					<Card.Root>
						<Card.Header>
							<Card.Title>ผู้ประเมิน</Card.Title>
							<Card.Description>ผู้ที่ได้รับมอบหมายให้ประเมินรายการนี้</Card.Description>
						</Card.Header>
						<Card.Content class="space-y-3">
							{#if observation.evaluators.length === 0}
								<PageState
									title="ยังไม่มีผู้ประเมิน"
									description="รายการนี้ยังไม่ได้มอบหมายผู้ประเมิน"
								/>
							{:else}
								{#each observation.evaluators as evaluator (evaluator.id)}
									<div
										class="flex flex-col gap-2 rounded-md border p-3 sm:flex-row sm:items-center sm:justify-between"
									>
										<div class="min-w-0">
											<p class="font-medium">{evaluator.evaluatorDisplayName ?? 'ผู้ประเมิน'}</p>
											<p class="text-sm text-muted-foreground">
												{evaluator.roleLabel ?? 'ผู้ประเมิน'} · {evaluator.isRequired
													? 'จำเป็น'
													: 'ไม่จำเป็น'}
											</p>
										</div>
										<div class="flex flex-wrap items-center gap-2">
											<Badge
												variant={evaluator.status === 'submitted'
													? 'default'
													: evaluator.status === 'draft'
														? 'secondary'
														: 'outline'}
											>
												{evaluatorStatusLabel(evaluator.status)}
											</Badge>
											<span class="text-sm text-muted-foreground">
												{formatDateTime(evaluator.submittedAt)}
											</span>
										</div>
									</div>
								{/each}
							{/if}
						</Card.Content>
					</Card.Root>

					<Card.Root>
						<Card.Header>
							<div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
								<div>
									<Card.Title>แบบประเมินและผล</Card.Title>
									<Card.Description>
										ตรวจคำตอบรายข้อ คะแนนเฉลี่ย และรับรอง/อนุมัติผลจากหน้านี้
									</Card.Description>
								</div>
								<div class="flex flex-wrap gap-2">
									{#if canCertifyResult}
										<LoadingButton
											variant="outline"
											onclick={certifyResult}
											loading={savingAction === `certify-result:${observation.id}`}
											loadingLabel="กำลังรับรอง..."
										>
											รับรองผล
										</LoadingButton>
									{/if}
									{#if canApproveResult}
										<LoadingButton
											variant="outline"
											onclick={approveResult}
											loading={savingAction === `approve-result:${observation.id}`}
											loadingLabel="กำลังอนุมัติ..."
										>
											อนุมัติผล
										</LoadingButton>
									{/if}
								</div>
							</div>
						</Card.Header>
						<Card.Content class="space-y-4">
							<div class="grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-4">
								<div class="rounded-md border bg-muted/20 p-3">
									<p class="text-xs text-muted-foreground">แบบประเมิน</p>
									<p class="font-medium">
										{review?.template.title ??
											template?.title ??
											(templateLoading ? 'กำลังโหลด...' : '-')}
									</p>
								</div>
								<div class="rounded-md border bg-muted/20 p-3">
									<p class="text-xs text-muted-foreground">คะแนนเฉลี่ยทั้งหมด</p>
									<p class="font-medium">
										{review
											? reviewAverageScoreLabel(review.averageRating)
											: observationAverageScoreLabel(observation)}
									</p>
								</div>
								<div class="rounded-md border bg-muted/20 p-3">
									<p class="text-xs text-muted-foreground">จำนวนผู้ประเมิน</p>
									<p class="font-medium">{observation.evaluators.length} คน</p>
								</div>
								<div class="rounded-md border bg-muted/20 p-3">
									<p class="text-xs text-muted-foreground">รอบนิเทศ</p>
									<p class="font-medium">{cycle?.title ?? (cycleLoading ? 'กำลังโหลด...' : '-')}</p>
									{#if cycle}<p class="mt-1 text-xs text-muted-foreground">
											{cycleDateRangeLabel(cycle.observationStartDate, cycle.observationEndDate)}
										</p>{/if}
								</div>
							</div>

							<div data-testid="supervision-review" aria-busy={loadingReview}>
								{#if reviewOpen}<Button variant="outline" onclick={() => (reviewOpen = false)}
										>ปิดผลรายข้อ</Button
									>{/if}
								{#if !canViewReviewDetails}
									<PageState
										title="ยังไม่เปิดเผยผลประเมินรายข้อ"
										description="ผลคะแนนรายข้อจะแสดงหลังหัวหน้ากลุ่มบริหารวิชาการอนุมัติผลแล้ว"
									/>
								{:else if !reviewOpen && reviewableStatus(observation.status)}
									<Button variant="outline" onclick={openReview}>ดูผลประเมินรายข้อ</Button>
								{:else if loadingReview && !review}
									<div role="status" aria-label="กำลังโหลดผลประเมิน">
										<PageSkeleton variant="detail" />
									</div>
								{:else if reviewError && !review}
									<div class="space-y-3">
										<PageState title="โหลดผลประเมินไม่สำเร็จ" description={reviewError} />
										<Button variant="outline" onclick={() => void loadReviewDetail()}>
											<RefreshCw class="h-4 w-4" />
											โหลดผลอีกครั้ง
										</Button>
									</div>
								{:else if review && reviewOpen}
									{#if reviewError}<PageState
											title="อัปเดตผลไม่สำเร็จ"
											description={reviewError}
										/><Button variant="outline" onclick={() => void loadReviewDetail()}
											>โหลดผลอีกครั้ง</Button
										>{/if}
									<div class="space-y-4" data-supervision-review-rubric="readonly">
										<div
											class="grid gap-3 rounded-md border bg-muted/20 p-3 text-sm sm:grid-cols-2 lg:grid-cols-4"
										>
											<div>
												<p class="text-xs text-muted-foreground">มุมมอง</p>
												<p class="font-medium">
													{selectedReviewEvaluator?.evaluatorDisplayName ??
														'สรุปเฉลี่ยทุกผู้ประเมิน'}
												</p>
											</div>
											<div>
												<p class="text-xs text-muted-foreground">คะแนนเฉลี่ยมุมมองนี้</p>
												<p class="font-medium">
													{selectedReviewEvaluator
														? reviewAverageScoreLabel(selectedReviewEvaluator.averageRating)
														: reviewAverageScoreLabel(review.averageRating)}
												</p>
											</div>
											<div>
												<p class="text-xs text-muted-foreground">ตอบแบบคะแนน</p>
												<p class="font-medium">
													{selectedReviewSummary?.answeredRatingCount ?? 0} /
													{selectedReviewSummary?.ratingItemCount ?? 0}
												</p>
											</div>
											<div>
												<p class="text-xs text-muted-foreground">ระดับคุณภาพ</p>
												<p class="font-medium">
													{selectedReviewSummary?.percentage === null ||
													selectedReviewSummary?.percentage === undefined
														? '-'
														: `${selectedReviewSummary.percentage.toFixed(2)}% · ${selectedReviewSummary.qualityLabel}`}
												</p>
											</div>
										</div>

										<div class="flex flex-wrap gap-2">
											<Button
												type="button"
												size="sm"
												variant={selectedReviewEvaluatorId === 'summary' ? 'default' : 'outline'}
												onclick={() => (selectedReviewEvaluatorId = 'summary')}
											>
												สรุปเฉลี่ย
											</Button>
											{#each review.evaluatorResults as evaluator (evaluator.evaluatorId)}
												<Button
													type="button"
													size="sm"
													variant={selectedReviewEvaluatorId === evaluator.evaluatorId
														? 'default'
														: 'outline'}
													onclick={() => (selectedReviewEvaluatorId = evaluator.evaluatorId)}
												>
													{evaluator.evaluatorDisplayName ?? 'ผู้ประเมิน'}
													<Badge variant="secondary" class="ml-1">
														{reviewAverageScoreLabel(evaluator.averageRating)}
													</Badge>
												</Button>
											{/each}
										</div>

										{#each reviewRubricSections as section (section.localId)}
											{@const progress = sectionRubricProgress(
												section,
												selectedReviewDrafts,
												review.template.ratingMax
											)}
											<div class="space-y-3 rounded-md border bg-background p-3">
												<div
													class="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between"
												>
													<div>
														<h3 class="text-sm font-semibold">{section.title}</h3>
														{#if section.description}
															<p class="text-xs text-muted-foreground">{section.description}</p>
														{/if}
													</div>
													<div class="flex flex-wrap gap-2">
														<Badge variant="outline">
															ตอบ {progress.answeredRatingCount}/{progress.ratingCount}
														</Badge>
														<Badge variant="outline">
															คะแนน {progress.totalScore.toFixed(2)}/{progress.maxScore}
														</Badge>
														<Badge variant="secondary">
															{progress.percentage === null ? '-' : progress.percentage.toFixed(2)}%
															·
															{qualityLevelFromPercentage(progress.percentage)}
														</Badge>
													</div>
												</div>
												<div class="overflow-x-auto rounded-md border">
													<Table.Root>
														<Table.Header>
															<Table.Row>
																<Table.Head class="min-w-72">หัวข้อประเมิน</Table.Head>
																{#each ratingScale(review.template.ratingMin, review.template.ratingMax) as score (score)}
																	<Table.Head class="w-14 text-center">{score}</Table.Head>
																{/each}
																<Table.Head class="min-w-40">ผล</Table.Head>
															</Table.Row>
														</Table.Header>
														<Table.Body>
															{#each section.items as item (item.localId)}
																{@const itemRating = reviewItemRatingFor(item)}
																<Table.Row>
																	<Table.Cell class="align-top">
																		<p class="font-medium">{item.label}</p>
																		{#if item.description}
																			<p class="text-xs text-muted-foreground">
																				{item.description}
																			</p>
																		{/if}
																	</Table.Cell>
																	{#if item.itemType === 'rating'}
																		{#each ratingScale(review.template.ratingMin, review.template.ratingMax) as score (score)}
																			<Table.Cell class="text-center align-middle">
																				<div
																					class={cn(
																						'mx-auto flex h-8 w-8 items-center justify-center rounded-md border text-xs',
																						selectedReviewEvaluator && itemRating === score
																							? 'border-primary bg-primary text-primary-foreground'
																							: !selectedReviewEvaluator &&
																								  itemRating !== null &&
																								  Math.round(itemRating) === score
																								? 'border-primary/60 bg-primary/10 text-primary'
																								: 'bg-muted/20 text-muted-foreground'
																					)}
																				>
																					{#if selectedReviewEvaluator && itemRating === score}
																						<Check class="h-4 w-4" />
																					{:else}
																						{score}
																					{/if}
																				</div>
																			</Table.Cell>
																		{/each}
																		<Table.Cell class="align-top">
																			{#if selectedReviewEvaluator}
																				<Badge variant="secondary">
																					คะแนน {reviewAverageScoreLabel(itemRating)}
																				</Badge>
																			{:else}
																				<div class="space-y-1">
																					<Badge variant="secondary">
																						เฉลี่ย {reviewAverageScoreLabel(itemRating)}
																					</Badge>
																					<p class="text-xs text-muted-foreground">
																						{reviewItemSummaryFor(item.localId)?.responseCount ?? 0} คำตอบ
																					</p>
																				</div>
																			{/if}
																		</Table.Cell>
																	{:else}
																		<Table.Cell
																			colspan={ratingScale(
																				review.template.ratingMin,
																				review.template.ratingMax
																			).length}
																			class="align-top"
																		>
																			<p class="whitespace-pre-wrap text-sm">
																				{reviewItemTextFor(item) || '-'}
																			</p>
																		</Table.Cell>
																		<Table.Cell class="align-top">
																			<Badge variant="outline">ข้อความ</Badge>
																		</Table.Cell>
																	{/if}
																</Table.Row>
															{/each}
														</Table.Body>
													</Table.Root>
												</div>
											</div>
										{/each}
									</div>
								{:else}
									<PageState
										title="ยังไม่มีผลประเมินสำหรับตรวจ"
										description="ผลรายข้อจะแสดงเมื่อผู้ประเมินส่งแบบประเมินครบและรายการเข้าสู่ขั้นตอนรับรองผล"
									/>
								{/if}
							</div>
						</Card.Content>
					</Card.Root>
				</div>

				<div class="min-w-0 space-y-4">
					<div
						class="space-y-3"
						hidden={!cycleError && (!cycleLoading || !!cycle)}
						data-testid="supervision-detail-cycle"
						aria-busy={cycleLoading}
					>
						{#if cycleLoading && !cycle}<div role="status" aria-label="กำลังโหลด">
								<PageSkeleton rows={1} columns={1} />
							</div>{/if}
						{#if cycleError}<PageState
								title="โหลดรอบนิเทศไม่สำเร็จ"
								description={cycleError}
							/><Button variant="outline" onclick={retryCycle}>ลองใหม่</Button>{/if}
					</div>
					<div
						class="space-y-3"
						hidden={!templateError && (!templateLoading || !!template)}
						data-testid="supervision-detail-template"
						aria-busy={templateLoading}
					>
						{#if templateLoading && !template}<div role="status" aria-label="กำลังโหลด">
								<PageSkeleton rows={1} columns={1} />
							</div>{/if}
						{#if templateError}<PageState
								title="โหลดข้อมูลแบบประเมินไม่สำเร็จ"
								description={templateError}
							/><Button variant="outline" onclick={retryTemplate}>ลองใหม่</Button>{/if}
					</div>
					<Card.Root>
						<Card.Header>
							<Card.Title>ภาพรวม</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-3 text-sm">
							<div>
								<p class="text-xs text-muted-foreground">ผู้รับการนิเทศ</p>
								<p class="font-medium">{observation.observedDisplayName ?? '-'}</p>
							</div>
							<div>
								<p class="text-xs text-muted-foreground">สถานะ</p>
								<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
							</div>
							<div>
								<p class="text-xs text-muted-foreground">รอบนิเทศ</p>
								<p class="font-medium">{cycle?.title ?? (cycleLoading ? 'กำลังโหลด...' : '-')}</p>
								{#if cycle}<p class="mt-1 text-xs text-muted-foreground">
										{cycleDateRangeLabel(cycle.observationStartDate, cycle.observationEndDate)}
									</p>{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground">ภาคเรียน/ปีการศึกษา</p>
								<p class="font-medium">{observationContextLabel(observation)}</p>
							</div>
						</Card.Content>
					</Card.Root>
				</div>
			</div>
		{/if}
	</div>
</PageShell>

<Dialog.Root bind:open={editLessonOpen}>
	<Dialog.Content class="flex max-h-[92vh] flex-col sm:max-w-6xl">
		<Dialog.Header
			><Dialog.Title>แก้คาบ/วันเวลา</Dialog.Title><Dialog.Description
				>เลือกช่วงคาบจากตารางสอนปัจจุบัน ข้อมูลคาบเดิมแสดงไว้เพื่ออ่านประกอบ</Dialog.Description
			></Dialog.Header
		>
		<div class="min-h-0 flex-1 space-y-4 overflow-y-auto pr-1">
			{#if observation && editLessonOpen}
				<SupervisionSavedPeriods {observation} />
				{#if cycle && !cycleDatesError}
					<SupervisionLessonPicker
						{cycle}
						academicYearId={observation.academicYearId}
						academicTermId={observation.academicTermId}
						observationId={observation.id}
						initialDate={schoolCivilDate(new Date(observation.observedAt))}
						allowed={canEditLesson}
						onreloadCycle={retryCycle}
						disabled={isReturnedOwnRequest && cycle?.status !== 'open'}
						onsubmit={saveLessonEdit}
						submitLabel={isReturnedOwnRequest ? 'แก้ไขและส่งใหม่' : 'บันทึกคาบนิเทศ'}
					/>
				{:else if cycleLoading}<PageSkeleton />{:else}<PageState
						title="รอผู้ดูแลกำหนดช่วงวันที่นิเทศ"
						description="ยังเลือกคาบใหม่ไม่ได้จนกว่ารอบนี้จะมีช่วงวันที่นิเทศ"
					/>{/if}
			{/if}
		</div>
		<Dialog.Footer
			><Button variant="outline" onclick={() => (editLessonOpen = false)}>ยกเลิก</Button
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root
	open={requestReviewOpen && canReviewPendingRequest}
	onOpenChange={setRequestReviewOpen}
>
	{#if observation && canReviewPendingRequest}
		<Dialog.Content class="max-h-[85dvh] overflow-y-auto sm:max-w-2xl">
			<Dialog.Header>
				<Dialog.Title>ตรวจคำขอจองนิเทศ</Dialog.Title>
				<Dialog.Description
					>{observation.observedDisplayName ?? 'ครูผู้ขอรับนิเทศ'}</Dialog.Description
				>
			</Dialog.Header>
			<SupervisionRequestReview
				{observation}
				review={requestReview}
				showSaveEvaluators
				showSchedule
				onclose={() => setRequestReviewOpen(false)}
			/>
		</Dialog.Content>
	{/if}
</Dialog.Root>

<Dialog.Root bind:open={editEvaluatorsOpen}>
	<Dialog.Content class="sm:max-w-2xl" aria-busy={loadingEvaluatorAvailability}>
		<Dialog.Header>
			<Dialog.Title>แก้ผู้ประเมิน</Dialog.Title>
			<Dialog.Description>ผู้ประเมินที่ส่งผลแล้วจะถูกเก็บไว้เพื่อรักษาผลประเมิน</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-3">
			{#if loadingEvaluatorAvailability}<div role="status" aria-label="กำลังโหลด">
					<PageSkeleton />
				</div>{/if}
			{#if evaluatorAvailabilityError}<PageState
					title="โหลดผู้ประเมินไม่สำเร็จ"
					description={evaluatorAvailabilityError}
				/><Button variant="outline" onclick={() => void loadEvaluatorAvailability(true)}
					>ลองใหม่</Button
				>{/if}
			<div class="flex min-h-10 flex-wrap items-center gap-2 rounded-md border p-2">
				{#if selectedEvaluators.length === 0}
					<span class="text-sm text-muted-foreground">ยังไม่ได้เลือกผู้ประเมิน</span>
				{:else}
					{#each selectedEvaluators as staff (staff.id)}
						<Badge variant="secondary" class="gap-1 pr-1">
							<span>{staff.name}</span>
							<button
								type="button"
								class="rounded-sm p-0.5 text-muted-foreground hover:bg-background hover:text-foreground"
								aria-label={`ลบผู้ประเมิน ${staff.name}`}
								onclick={() => removeEvaluator(staff.id)}
							>
								<Trash2 class="h-3 w-3" />
							</button>
						</Badge>
					{/each}
				{/if}
			</div>

			<Popover.Root bind:open={evaluatorPickerOpen}>
				<Popover.Trigger>
					{#snippet child({ props })}
						<Button
							type="button"
							variant="outline"
							role="combobox"
							aria-label="เพิ่ม/เลือกผู้ประเมิน"
							aria-expanded={evaluatorPickerOpen}
							class="w-full justify-between font-normal"
							disabled={loadingEvaluatorAvailability ||
								!evaluatorAvailabilityLoaded ||
								Boolean(evaluatorAvailabilityError)}
							{...props}
						>
							<span class="truncate">
								{loadingEvaluatorAvailability
									? 'กำลังตรวจผู้ประเมินที่ว่าง...'
									: 'เพิ่ม/เลือกผู้ประเมิน'}
							</span>
							<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
						</Button>
					{/snippet}
				</Popover.Trigger>
				<Popover.Content class="w-[--bits-popover-trigger-width] p-0">
					<Command.Root>
						<Command.Input placeholder="ค้นหาครูผู้ประเมินที่ว่าง..." />
						<Command.Empty>ไม่พบครูผู้ประเมินที่ว่าง</Command.Empty>
						<Command.List class="max-h-72">
							<Command.Group>
								{#each availableEvaluators.filter((evaluator) => evaluator.available) as staff (staff.id)}
									<Command.Item
										value={`${staff.name} ${staff.title ?? ''} ${staff.id}`}
										onSelect={() => toggleEvaluator(staff)}
									>
										<Check
											class={cn(
												'mr-2 h-4 w-4',
												selectedEvaluatorIds.includes(staff.id) ? 'opacity-100' : 'opacity-0'
											)}
										/>
										<span>{staff.name}</span>
									</Command.Item>
								{/each}
							</Command.Group>
						</Command.List>
					</Command.Root>
				</Popover.Content>
			</Popover.Root>
			{#if unavailableEvaluatorCount > 0}
				<p class="text-xs text-muted-foreground">
					ผู้ประเมินไม่ว่าง {unavailableEvaluatorCount} คน เพราะมีนัดนิเทศที่ยืนยันแล้วในช่วงคาบนี้
				</p>
			{/if}
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (editEvaluatorsOpen = false)}>ยกเลิก</Button>
			<LoadingButton
				onclick={saveEvaluatorEdit}
				disabled={loadingEvaluatorAvailability ||
					!evaluatorAvailabilityLoaded ||
					Boolean(evaluatorAvailabilityError)}
				loading={savingAction === 'evaluators'}
				loadingLabel="กำลังบันทึก..."
			>
				บันทึกผู้ประเมิน
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={cancelDialogOpen}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>ยกเลิกรายการนิเทศ</Dialog.Title>
			<Dialog.Description>รายการจะเปลี่ยนเป็นยกเลิกและหยุดดำเนินการต่อ</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-2">
			<Label>เหตุผล</Label>
			<Textarea bind:value={cancelReason} rows={3} placeholder="ระบุเหตุผลการยกเลิก" />
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (cancelDialogOpen = false)}>ไม่ยกเลิก</Button>
			<LoadingButton
				variant="destructive"
				onclick={cancelObservation}
				loading={savingAction === 'cancel'}
				disabled={savingAction !== null}
				loadingLabel="กำลังยกเลิก..."
			>
				ยืนยันยกเลิก
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
