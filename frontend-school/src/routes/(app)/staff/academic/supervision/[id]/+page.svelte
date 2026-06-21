<script lang="ts">
	import { onMount } from 'svelte';
	import {
		ArrowLeft,
		CalendarClock,
		Check,
		ChevronsUpDown,
		RefreshCw,
		Trash2,
		UserCheck
	} from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import {
		approveSupervisionObservation,
		cancelRequestedSupervisionObservation,
		cancelSupervisionObservation,
		certifySupervisionObservation,
		getSupervisionEvaluatorAvailability,
		getSupervisionObservation,
		getSupervisionObservationReview,
		getSupervisionObservationTimetableOptions,
		getSupervisionTemplate,
		listSupervisionCycles,
		replaceSupervisionObservationEvaluators,
		updateRequestedSupervisionObservation,
		updateSupervisionObservation,
		type ManualLesson,
		type SupervisionCycle,
		type SupervisionEvaluatorAvailability,
		type SupervisionEvaluator,
		type SupervisionObservation,
		type SupervisionObservationReview,
		type SupervisionObservationStatus,
		type SupervisionReviewEvaluatorResult,
		type SupervisionReviewResponse,
		type SupervisionTemplate
	} from '$lib/api/supervision';
	import type { TimetableEntry } from '$lib/api/timetable';
	import {
		calculateRubricDraftSummary,
		qualityLevelFromPercentage,
		sectionRubricProgress,
		type RubricFormSection,
		type RubricResponseDraft
	} from '$lib/utils/supervision-rubric';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import { cn } from '$lib/utils';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Command from '$lib/components/ui/command';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Popover from '$lib/components/ui/popover';
	import * as Table from '$lib/components/ui/table';
	import { Textarea } from '$lib/components/ui/textarea';
	import type { PageData } from './$types';

	type LessonEditForm = {
		subjectName: string;
		classroomLabel: string;
		roomLabel: string;
		periodLabel: string;
		observedDate: string;
		observedTime: string;
		reason: string;
	};

	type DetailItem = {
		label: string;
		value: string;
	};

	let { data }: { data: PageData } = $props();

	let loading = $state(true);
	let savingAction = $state<string | null>(null);
	let error = $state('');
	let observation = $state<SupervisionObservation | null>(null);
	let review = $state<SupervisionObservationReview | null>(null);
	let template = $state<SupervisionTemplate | null>(null);
	let cycles = $state<SupervisionCycle[]>([]);
	let availableEvaluators = $state<SupervisionEvaluatorAvailability[]>([]);
	let loadingReview = $state(false);
	let reviewError = $state('');
	let selectedReviewEvaluatorId = $state('summary');
	let loadingEvaluatorAvailability = $state(false);
	let editTimetableEntries = $state<TimetableEntry[]>([]);
	let loadingEditTimetable = $state(false);
	let selectedEditTimetableEntryId = $state('');
	let selectedEditTimetableDate = $state('');
	let editWeekStartDate = $state('');
	let editLessonOpen = $state(false);
	let editEvaluatorsOpen = $state(false);
	let evaluatorPickerOpen = $state(false);
	let cancelDialogOpen = $state(false);
	let cancelReason = $state('');
	let selectedEvaluatorIds = $state<string[]>([]);
	let lessonForm = $state<LessonEditForm>({
		subjectName: '',
		classroomLabel: '',
		roomLabel: '',
		periodLabel: '',
		observedDate: '',
		observedTime: '',
		reason: ''
	});

	const currentUserId = $derived($authStore.user?.id ?? '');
	const canManageObservation = $derived(
		$can.hasAny(
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.SUPERVISION_MANAGE_SCHOOL
		)
	);
	const canApproveSchool = $derived($can.has(PERMISSIONS.SUPERVISION_APPROVE_SCHOOL));
	const canViewReviewDetails = $derived(
		Boolean(
			observation && (actorCanRequestReview() || observationResultsReleased(observation.status))
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
	const canEditOwnRequested = $derived(
		Boolean(
			observation &&
			observation.observedUserId === currentUserId &&
			observation.status === 'requested' &&
			$can.has(PERMISSIONS.SUPERVISION_REQUEST_OWN)
		)
	);
	const canEditLesson = $derived(
		Boolean(
			observation &&
			manageableStatus(observation.status) &&
			(canManageObservation || canEditOwnRequested)
		)
	);
	const canEditEvaluators = $derived(
		Boolean(observation && manageableStatus(observation.status) && canManageObservation)
	);
	const canCancelObservation = $derived(
		Boolean(
			observation &&
			(canEditOwnRequested ||
				(canManageObservation &&
					!['published', 'acknowledged', 'completed', 'cancelled'].includes(observation.status)))
		)
	);
	const cycle = $derived(cycles.find((item) => item.id === observation?.cycleId) ?? null);
	const pageTitle = $derived(observation?.observedDisplayName ?? 'รายละเอียดรายการนิเทศ');
	const selectedEvaluators = $derived(
		availableEvaluators.filter((staff) => selectedEvaluatorIds.includes(staff.id))
	);
	const unavailableEvaluatorCount = $derived(
		availableEvaluators.filter((evaluator) => !evaluator.available).length
	);
	const selectedEditTimetableEntry = $derived(
		editTimetableEntries.find((entry) => entry.id === selectedEditTimetableEntryId) ?? null
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

	onMount(loadPage);

	function manageableStatus(status: SupervisionObservationStatus): boolean {
		return status === 'requested' || status === 'planned' || status === 'returned';
	}

	function statusLabel(status: SupervisionObservationStatus): string {
		return (
			(
				{
					requested: 'รออนุมัติคำขอ',
					planned: 'วางแผนแล้ว',
					in_progress: 'กำลังประเมิน',
					evaluators_submitted: 'รอรับรองผล',
					under_review: 'รอรับรองผล',
					returned: 'ส่งกลับคำขอ',
					approved: 'รออนุมัติผล',
					published: 'รอครูรับทราบ',
					acknowledged: 'เสร็จสิ้น',
					completed: 'เสร็จสิ้น',
					cancelled: 'ยกเลิก'
				} satisfies Record<SupervisionObservationStatus, string>
			)[status] ?? status
		);
	}

	function observationResultsReleased(status: SupervisionObservationStatus): boolean {
		return status === 'published' || status === 'acknowledged' || status === 'completed';
	}

	function observationAverageScoreLabel(item: SupervisionObservation): string {
		if (item.averageRating !== null && item.averageRating !== undefined) {
			return item.averageRating.toFixed(2);
		}
		if (!observationResultsReleased(item.status)) {
			return 'รอหัวหน้ากลุ่มบริหารวิชาการอนุมัติผล';
		}
		return '-';
	}

	function actorCanRequestReview(): boolean {
		return $can.hasAny(
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.SUPERVISION_MANAGE_SCHOOL,
			PERMISSIONS.SUPERVISION_APPROVE_SCHOOL
		);
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

	function actionKindLabel(actionKind: string): string {
		return (
			{
				requested: 'ส่งคำขอ',
				request_cancelled: 'ยกเลิกคำขอ',
				updated: 'แก้ไขรายการ',
				evaluators_updated: 'แก้ไขผู้ประเมิน',
				planned: 'วางแผน/อนุมัติคำขอ',
				returned: 'ส่งกลับคำขอ',
				request_returned: 'ส่งกลับคำขอ',
				evaluator_draft_saved: 'บันทึกผลประเมินชั่วคราว',
				evaluator_submitted: 'ส่งผลประเมิน',
				submitted_for_review: 'เข้าคิวรับรองผล',
				subject_group_certified: 'รับรองผล',
				academic_approved: 'อนุมัติผล',
				approved: 'รับรองผล',
				result_approved: 'อนุมัติผล',
				published: 'อนุมัติผล',
				result_published: 'อนุมัติผล',
				acknowledged: 'รับทราบผล',
				result_acknowledged: 'รับทราบผล',
				cancelled: 'ยกเลิกรายการ'
			}[actionKind] ?? actionKind
		);
	}

	function formatDateTime(value?: string | null): string {
		if (!value) return '-';
		return new Date(value).toLocaleString('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short'
		});
	}

	function formatDateInput(value?: string | null): string {
		if (!value) return '';
		const date = new Date(value);
		const year = date.getFullYear();
		const month = String(date.getMonth() + 1).padStart(2, '0');
		const day = String(date.getDate()).padStart(2, '0');
		return `${year}-${month}-${day}`;
	}

	function formatTimeInput(value?: string | null): string {
		if (!value) return '08:30';
		const date = new Date(value);
		const hour = String(date.getHours()).padStart(2, '0');
		const minute = String(date.getMinutes()).padStart(2, '0');
		return `${hour}:${minute}`;
	}

	function toIsoDateTime(date: string, time: string): string {
		return new Date(`${date}T${time || '00:00'}`).toISOString();
	}

	function parseLocalDate(date: string): Date {
		const [year = '1970', month = '1', day = '1'] = date.split('-');
		return new Date(Number(year), Number(month) - 1, Number(day));
	}

	function toLocalDateInputValue(date: Date): string {
		const year = date.getFullYear();
		const month = String(date.getMonth() + 1).padStart(2, '0');
		const day = String(date.getDate()).padStart(2, '0');
		return `${year}-${month}-${day}`;
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

	function editTimetableDayValues(): string[] {
		const order: TimetableEntry['day_of_week'][] = [
			'MON',
			'TUE',
			'WED',
			'THU',
			'FRI',
			'SAT',
			'SUN'
		];
		const days = new Set(editTimetableEntries.map((entry) => entry.day_of_week));
		return order.filter((day) => days.has(day));
	}

	function editTimetableDayLabel(day: string): string {
		return (
			{
				MON: 'จันทร์',
				TUE: 'อังคาร',
				WED: 'พุธ',
				THU: 'พฤหัสบดี',
				FRI: 'ศุกร์',
				SAT: 'เสาร์',
				SUN: 'อาทิตย์'
			}[day] ?? day
		);
	}

	function dateForEditTimetableDay(day: string): string {
		const offsets: Record<string, number> = {
			MON: 0,
			TUE: 1,
			WED: 2,
			THU: 3,
			FRI: 4,
			SAT: 5,
			SUN: 6
		};
		const weekStart = editWeekStartDate || toLocalDateInputValue(startOfWeek(new Date()));
		return toLocalDateInputValue(addDays(parseLocalDate(weekStart), offsets[day] ?? 0));
	}

	function formatShortDate(date: string): string {
		return new Intl.DateTimeFormat('th-TH', {
			day: 'numeric',
			month: 'short'
		}).format(parseLocalDate(date));
	}

	function editTimetableObservedAt(entry: TimetableEntry, observedDate: string): string {
		const startTime = entry.start_time?.slice(0, 5) || '08:00';
		return toIsoDateTime(observedDate, startTime);
	}

	function editDateInCycle(date: string): boolean {
		if (!cycle) return true;
		return date >= formatDateInput(cycle.startsAt) && date <= formatDateInput(cycle.endsAt);
	}

	function editTimetablePeriodKey(entry: TimetableEntry): string {
		return (
			entry.period_id ||
			`${entry.start_time ?? ''}-${entry.end_time ?? ''}-${entry.period_name ?? ''}`
		);
	}

	function editTimetablePeriodLabel(entry: TimetableEntry): string {
		return entry.period_name || entry.title || 'ไม่ระบุคาบ';
	}

	function editTimetableTimeLabel(entry: TimetableEntry): string {
		if (!entry.start_time && !entry.end_time) return '';
		return `${entry.start_time?.slice(0, 5) ?? ''}-${entry.end_time?.slice(0, 5) ?? ''}`;
	}

	function editTimetablePeriodSort(entry: TimetableEntry): number {
		if (typeof entry.period_order_index === 'number') return entry.period_order_index;
		if (entry.start_time) {
			const [hour = '0', minute = '0'] = entry.start_time.split(':');
			return Number(hour) * 60 + Number(minute);
		}
		return 9999;
	}

	function editTimetablePeriodRows() {
		const rows: { key: string; label: string; timeLabel: string; sort: number }[] = [];
		for (const entry of editTimetableEntries) {
			const key = editTimetablePeriodKey(entry);
			if (!rows.some((row) => row.key === key)) {
				rows.push({
					key,
					label: editTimetablePeriodLabel(entry),
					timeLabel: editTimetableTimeLabel(entry),
					sort: editTimetablePeriodSort(entry)
				});
			}
		}
		return rows.sort(
			(left, right) => left.sort - right.sort || left.label.localeCompare(right.label)
		);
	}

	function editTimetableEntryFor(day: string, row: { key: string }): TimetableEntry | null {
		return (
			editTimetableEntries.find(
				(entry) => entry.day_of_week === day && editTimetablePeriodKey(entry) === row.key
			) ?? null
		);
	}

	function editTimetableEntryTitle(entry: TimetableEntry): string {
		return entry.subject_name_th || entry.title || entry.subject_code || 'คาบสอน';
	}

	function editTimetableEntryMeta(entry: TimetableEntry): string {
		const classroom = entry.classroom_name ?? '';
		const room = entry.room_code ? `ห้อง ${entry.room_code}` : '';
		return [classroom, room].filter(Boolean).join(' · ') || 'ไม่มีรายละเอียดเพิ่มเติม';
	}

	function selectLessonTimetableEntry(entry: TimetableEntry, observedDate: string) {
		if (!editDateInCycle(observedDate)) return;
		selectedEditTimetableEntryId = entry.id;
		selectedEditTimetableDate = observedDate;
		lessonForm.subjectName = editTimetableEntryTitle(entry);
		lessonForm.classroomLabel = entry.classroom_name ?? '';
		lessonForm.roomLabel = entry.room_code ?? '';
		lessonForm.periodLabel = entry.period_name ?? entry.title ?? '';
		lessonForm.observedDate = observedDate;
		lessonForm.observedTime = entry.start_time?.slice(0, 5) || '08:00';
	}

	function observationSubjectLabel(item: SupervisionObservation): string {
		return item.lessonSnapshot.subjectName ?? item.manualLesson?.subjectName ?? 'ไม่ระบุวิชา';
	}

	function observationClassroomLabel(item: SupervisionObservation): string {
		return item.lessonSnapshot.classroomLabel ?? item.manualLesson?.classroomLabel ?? '-';
	}

	function observationRoomLabel(item: SupervisionObservation): string {
		return item.lessonSnapshot.roomLabel ?? item.manualLesson?.roomLabel ?? '-';
	}

	function observationPeriodLabel(item: SupervisionObservation): string {
		return item.lessonSnapshot.periodLabel ?? item.manualLesson?.periodLabel ?? '-';
	}

	function lessonDetails(item: SupervisionObservation): DetailItem[] {
		return [
			{ label: 'วิชา', value: observationSubjectLabel(item) },
			{ label: 'คาบ', value: observationPeriodLabel(item) },
			{ label: 'ชั้น/ห้อง', value: observationClassroomLabel(item) },
			{ label: 'ห้องเรียน', value: observationRoomLabel(item) },
			{ label: 'วันที่/เวลา', value: formatDateTime(item.observedAt) },
			{ label: 'ที่มา', value: item.timetableEntryId ? 'ตารางสอน' : 'กำหนดเอง' }
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

	function replaceObservation(updated: SupervisionObservation) {
		observation = updated;
	}

	async function loadPage() {
		loading = true;
		error = '';
		review = null;
		reviewError = '';
		try {
			const loadedObservation = await getSupervisionObservation(data.observationId);
			observation = loadedObservation;
			const [cycleItems, loadedTemplate] = await Promise.all([
				listSupervisionCycles(),
				getSupervisionTemplate(loadedObservation.templateId)
			]);
			cycles = cycleItems;
			template = loadedTemplate;
			if (
				(actorCanRequestReview() || observationResultsReleased(loadedObservation.status)) &&
				reviewableStatus(loadedObservation.status)
			) {
				await loadReviewDetail(loadedObservation.id);
			}
		} catch (loadError) {
			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดรายการนิเทศได้';
			toast.error(error);
		} finally {
			loading = false;
		}
	}

	async function loadReviewDetail(id = observation?.id) {
		if (!id || !canViewReviewDetails) return;
		loadingReview = true;
		reviewError = '';
		try {
			const loadedReview = await getSupervisionObservationReview(id);
			review = loadedReview;
			observation = loadedReview.observation;
			template = loadedReview.template;
			if (
				selectedReviewEvaluatorId !== 'summary' &&
				!loadedReview.evaluatorResults.some(
					(result) => result.evaluatorId === selectedReviewEvaluatorId
				)
			) {
				selectedReviewEvaluatorId = 'summary';
			}
		} catch (loadError) {
			reviewError =
				loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดผลประเมินนิเทศได้';
		} finally {
			loadingReview = false;
		}
	}

	function replaceReviewObservation(updated: SupervisionObservation) {
		replaceObservation(updated);
		if (review) {
			review = {
				...review,
				observation: updated,
				averageRating: updated.averageRating ?? review.averageRating
			};
		}
	}

	async function certifyResult() {
		if (!observation || !canCertifyResult) return;
		savingAction = `certify-result:${observation.id}`;
		try {
			const response = await certifySupervisionObservation(observation.id);
			const updated = mutationData(response, 'รับรองผลไม่สำเร็จ');
			replaceReviewObservation(updated);
			await loadReviewDetail(updated.id);
			toast.success('รับรองผลนิเทศแล้ว');
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'รับรองผลไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function approveResult() {
		if (!observation || !canApproveResult) return;
		savingAction = `approve-result:${observation.id}`;
		try {
			const response = await approveSupervisionObservation(observation.id);
			const updated = mutationData(response, 'อนุมัติผลไม่สำเร็จ');
			replaceReviewObservation(updated);
			await loadReviewDetail(updated.id);
			toast.success('อนุมัติผลนิเทศแล้ว');
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'อนุมัติผลไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function loadEditTimetableOptions(force = false) {
		if (!observation || !canEditLesson) return;
		if (!force && editTimetableEntries.length > 0) return;
		loadingEditTimetable = true;
		try {
			editTimetableEntries = await getSupervisionObservationTimetableOptions(observation.id);
		} catch (loadError) {
			toast.error(loadError instanceof Error ? loadError.message : 'โหลดคาบสอนไม่สำเร็จ');
		} finally {
			loadingEditTimetable = false;
		}
	}

	async function openLessonEditor() {
		if (!observation || !canEditLesson) return;
		selectedEditTimetableEntryId = observation.timetableEntryId ?? '';
		selectedEditTimetableDate = formatDateInput(observation.observedAt);
		editWeekStartDate = toLocalDateInputValue(startOfWeek(new Date(observation.observedAt)));
		lessonForm = {
			subjectName: observationSubjectLabel(observation),
			classroomLabel: observationClassroomLabel(observation),
			roomLabel: observationRoomLabel(observation) === '-' ? '' : observationRoomLabel(observation),
			periodLabel: observationPeriodLabel(observation),
			observedDate: formatDateInput(observation.observedAt),
			observedTime: formatTimeInput(observation.observedAt),
			reason: observation.manualLesson?.reason ?? 'แก้ไขรายการนิเทศจากหน้ารายละเอียด'
		};
		await loadEditTimetableOptions(true);
		editLessonOpen = true;
	}

	async function saveLessonEdit() {
		if (!observation || !canEditLesson) return;
		if (selectedEditTimetableEntry && !selectedEditTimetableDate) {
			toast.error('เลือกวันที่จากตารางสอนก่อน');
			return;
		}
		if (
			!selectedEditTimetableEntry &&
			(!lessonForm.subjectName ||
				!lessonForm.classroomLabel ||
				!lessonForm.periodLabel ||
				!lessonForm.observedDate)
		) {
			toast.error('กรอกวิชา ชั้น/ห้อง คาบ และวันที่ให้ครบ');
			return;
		}
		const payload =
			selectedEditTimetableEntry && selectedEditTimetableDate
				? {
						timetableEntryId: selectedEditTimetableEntryId,
						observedAt: editTimetableObservedAt(
							selectedEditTimetableEntry,
							selectedEditTimetableDate
						),
						manualLesson: null
					}
				: {
						manualLesson: {
							subjectName: lessonForm.subjectName,
							classroomLabel: lessonForm.classroomLabel,
							roomLabel: lessonForm.roomLabel || null,
							periodLabel: lessonForm.periodLabel,
							observedAt: toIsoDateTime(lessonForm.observedDate, lessonForm.observedTime),
							reason: lessonForm.reason || 'แก้ไขรายการนิเทศ'
						} satisfies ManualLesson
					};

		savingAction = 'lesson';
		try {
			const response = canManageObservation
				? await updateSupervisionObservation(observation.id, payload)
				: await updateRequestedSupervisionObservation(observation.id, payload);
			const updated = mutationData(response, 'แก้ไขรายการนิเทศไม่สำเร็จ');
			replaceObservation(updated);
			editLessonOpen = false;
			toast.success('แก้ไขคาบนิเทศแล้ว');
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'แก้ไขรายการนิเทศไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function openEvaluatorEditor() {
		if (!observation || !canEditEvaluators) return;
		await loadEvaluatorAvailability(true);
		selectedEvaluatorIds = observation.evaluators.map((evaluator) => evaluator.evaluatorUserId);
		editEvaluatorsOpen = true;
	}

	async function loadEvaluatorAvailability(force = false) {
		if (!observation || !canEditEvaluators) return;
		if (!force && availableEvaluators.length > 0) return;
		loadingEvaluatorAvailability = true;
		try {
			const items = await getSupervisionEvaluatorAvailability(observation.id);
			const availableIds = new Set(
				items.filter((evaluator) => evaluator.available).map((evaluator) => evaluator.id)
			);
			availableEvaluators = items;
			selectedEvaluatorIds = selectedEvaluatorIds.filter((id) => availableIds.has(id));
		} catch (loadError) {
			toast.error(
				loadError instanceof Error ? loadError.message : 'ไม่สามารถตรวจสอบผู้ประเมินที่ว่างได้'
			);
		} finally {
			loadingEvaluatorAvailability = false;
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
		if (!observation || !canEditEvaluators) return;
		if (selectedEvaluatorIds.length === 0) {
			toast.error('เลือกผู้ประเมินอย่างน้อย 1 คน');
			return;
		}
		savingAction = 'evaluators';
		try {
			const response = await replaceSupervisionObservationEvaluators(observation.id, {
				evaluators: selectedEvaluatorIds.map((evaluatorUserId) => ({
					evaluatorUserId,
					isRequired: true
				}))
			});
			const updated = mutationData(response, 'แก้ไขผู้ประเมินไม่สำเร็จ');
			replaceObservation(updated);
			editEvaluatorsOpen = false;
			toast.success('แก้ไขผู้ประเมินแล้ว');
		} catch (saveError) {
			void loadEvaluatorAvailability(true);
			toast.error(saveError instanceof Error ? saveError.message : 'แก้ไขผู้ประเมินไม่สำเร็จ');
		} finally {
			savingAction = null;
		}
	}

	async function cancelObservation() {
		if (!observation || !canCancelObservation) return;
		savingAction = 'cancel';
		try {
			const response =
				canManageObservation && !canEditOwnRequested
					? await cancelSupervisionObservation(observation.id, { reason: cancelReason || null })
					: await cancelRequestedSupervisionObservation(observation.id);
			const updated = mutationData(response, 'ยกเลิกรายการนิเทศไม่สำเร็จ');
			replaceObservation(updated);
			cancelDialogOpen = false;
			cancelReason = '';
			toast.success('ยกเลิกรายการนิเทศแล้ว');
		} catch (cancelError) {
			toast.error(
				cancelError instanceof Error ? cancelError.message : 'ยกเลิกรายการนิเทศไม่สำเร็จ'
			);
		} finally {
			savingAction = null;
		}
	}
</script>

<PageShell
	title={pageTitle}
	description="รายละเอียดคาบนิเทศ ผู้ประเมิน ผลประเมิน และการจัดการรายการเดียว"
	backHref="/staff/academic/supervision"
	backLabel="กลับหน้านิเทศการสอน"
	contentClass="max-w-6xl"
>
	{#snippet actions()}
		<Button variant="outline" href="/staff/academic/supervision">
			<ArrowLeft class="h-4 w-4" />
			กลับ
		</Button>
		<Button variant="outline" onclick={loadPage} disabled={loading}>
			<RefreshCw class="h-4 w-4" />
			รีเฟรช
		</Button>
	{/snippet}

	{#if loading}
		<PageSkeleton />
	{:else if error}
		<PageState title="โหลดรายการนิเทศไม่สำเร็จ" description={error} />
	{:else if !observation}
		<PageState title="ไม่พบรายการนิเทศ" description="รายการนี้อาจถูกลบหรือคุณไม่มีสิทธิ์เข้าถึง" />
	{:else}
		<div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
			<div class="space-y-4">
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
							</div>
							<div class="flex flex-wrap gap-2">
								{#if canEditLesson}
									<Button variant="outline" onclick={() => void openLessonEditor()}>
										<CalendarClock class="h-4 w-4" />
										แก้คาบ/วันเวลา
									</Button>
								{/if}
								{#if canEditEvaluators}
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
					<Card.Content>
						<div class="grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-3">
							{#each lessonDetails(observation) as detail (detail.label)}
								<div class="rounded-md border bg-muted/20 p-3">
									<p class="text-xs text-muted-foreground">{detail.label}</p>
									<p class="font-medium">{detail.value}</p>
								</div>
							{/each}
						</div>
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
									{review?.template.title ?? template?.title ?? 'ไม่พบแบบประเมิน'}
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
								<p class="font-medium">{cycle?.title ?? '-'}</p>
							</div>
						</div>

						{#if !canViewReviewDetails}
							<PageState
								title="ยังไม่เปิดเผยผลประเมินรายข้อ"
								description="ผลคะแนนรายข้อจะแสดงหลังหัวหน้ากลุ่มบริหารวิชาการอนุมัติผลแล้ว"
							/>
						{:else if loadingReview}
							<PageState
								title="กำลังโหลดผลประเมิน"
								description="กำลังดึงคำตอบรายข้อจากผู้ประเมิน"
							/>
						{:else if reviewError}
							<div class="space-y-3">
								<PageState title="โหลดผลประเมินไม่สำเร็จ" description={reviewError} />
								<Button variant="outline" onclick={() => void loadReviewDetail()}>
									<RefreshCw class="h-4 w-4" />
									โหลดผลอีกครั้ง
								</Button>
							</div>
						{:else if review}
							<div class="space-y-4" data-supervision-review-rubric="readonly">
								<div
									class="grid gap-3 rounded-md border bg-muted/20 p-3 text-sm sm:grid-cols-2 lg:grid-cols-4"
								>
									<div>
										<p class="text-xs text-muted-foreground">มุมมอง</p>
										<p class="font-medium">
											{selectedReviewEvaluator?.evaluatorDisplayName ?? 'สรุปเฉลี่ยทุกผู้ประเมิน'}
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
										<div class="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
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
													{progress.percentage === null ? '-' : progress.percentage.toFixed(2)}% ·
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
																	<p class="text-xs text-muted-foreground">{item.description}</p>
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
					</Card.Content>
				</Card.Root>
			</div>

			<div class="space-y-4">
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
							<p class="font-medium">{cycle?.title ?? '-'}</p>
						</div>
						<div>
							<p class="text-xs text-muted-foreground">ภาคเรียน/ปีการศึกษา</p>
							<p class="font-medium">
								{cycle ? `${cycle.semester}/${cycle.academicYear}` : '-'}
							</p>
						</div>
					</Card.Content>
				</Card.Root>

				<Card.Root>
					<Card.Header>
						<Card.Title>ประวัติ</Card.Title>
						<Card.Description>ลำดับการทำงานที่บันทึกจากระบบ</Card.Description>
					</Card.Header>
					<Card.Content class="space-y-3">
						{#if observation.actions.length === 0}
							<PageState
								title="ยังไม่มีประวัติ"
								description="เมื่อมีการดำเนินการ ระบบจะแสดงประวัติที่นี่"
							/>
						{:else}
							{#each observation.actions as action (action.id)}
								<div class="flex items-start gap-3 rounded-md border p-3">
									<div class="mt-1 h-2 w-2 shrink-0 rounded-full bg-primary"></div>
									<div class="min-w-0 space-y-1">
										<div class="flex flex-wrap items-center gap-2">
											<p class="text-sm font-medium">{actionKindLabel(action.actionKind)}</p>
											{#if action.fromStatus || action.toStatus}
												<Badge variant="outline">
													{action.fromStatus ? statusLabel(action.fromStatus) : '-'} → {action.toStatus
														? statusLabel(action.toStatus)
														: '-'}
												</Badge>
											{/if}
										</div>
										<p class="text-sm text-muted-foreground">
											{action.actorDisplayName ?? 'ระบบ'} · {formatDateTime(action.createdAt)}
										</p>
										{#if action.comment}
											<p class="text-sm">{action.comment}</p>
										{/if}
									</div>
								</div>
							{/each}
						{/if}
					</Card.Content>
				</Card.Root>
			</div>
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={editLessonOpen}>
	<Dialog.Content class="flex max-h-[92vh] flex-col sm:max-w-5xl">
		<Dialog.Header>
			<Dialog.Title>แก้คาบ/วันเวลา</Dialog.Title>
			<Dialog.Description>
				เลือกคาบจากตารางสอนของครูผู้ถูกนิเทศ หรือระบุคาบกำหนดเองเมื่อไม่มีในตาราง
			</Dialog.Description>
		</Dialog.Header>
		<div class="min-h-0 flex-1 space-y-4 overflow-y-auto pr-1">
			<div
				class="flex flex-col gap-2 rounded-md border bg-muted/20 p-3 sm:flex-row sm:items-center sm:justify-between"
			>
				<div>
					<p class="text-sm font-medium">สัปดาห์ {formatShortDate(editWeekStartDate)}</p>
					<p class="text-xs text-muted-foreground">
						แสดงเฉพาะวันที่มีคาบสอนของครูผู้ถูกนิเทศในตาราง
					</p>
				</div>
				<div class="flex flex-wrap gap-2">
					<Button
						type="button"
						size="sm"
						variant="outline"
						onclick={() => (editWeekStartDate = addWeeks(editWeekStartDate, -1))}
					>
						สัปดาห์ก่อน
					</Button>
					<Button
						type="button"
						size="sm"
						variant="outline"
						onclick={() => (editWeekStartDate = toLocalDateInputValue(startOfWeek(new Date())))}
					>
						สัปดาห์นี้
					</Button>
					<Button
						type="button"
						size="sm"
						variant="outline"
						onclick={() => (editWeekStartDate = addWeeks(editWeekStartDate, 1))}
					>
						สัปดาห์ถัดไป
					</Button>
				</div>
			</div>

			{#if loadingEditTimetable}
				<PageState title="กำลังโหลดตารางสอน" description="กำลังดึงคาบสอนสำหรับแก้ไขรายการนิเทศ" />
			{:else if editTimetableEntries.length === 0}
				<PageState
					title="ไม่พบคาบสอนในตาราง"
					description="ใช้คาบกำหนดเองด้านล่างเมื่อคาบนิเทศไม่ได้อยู่ในตารางสอน"
				/>
			{:else}
				<div class="overflow-x-auto rounded-md border">
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head class="w-36">คาบ</Table.Head>
								{#each editTimetableDayValues() as day (day)}
									<Table.Head class="min-w-40">
										<div class="space-y-1">
											<p>{editTimetableDayLabel(day)}</p>
											<p class="text-xs font-normal text-muted-foreground">
												{formatShortDate(dateForEditTimetableDay(day))}
											</p>
										</div>
									</Table.Head>
								{/each}
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each editTimetablePeriodRows() as row (row.key)}
								<Table.Row>
									<Table.Cell class="align-top">
										<p class="font-medium">{row.label}</p>
										{#if row.timeLabel}
											<p class="text-xs text-muted-foreground">{row.timeLabel}</p>
										{/if}
									</Table.Cell>
									{#each editTimetableDayValues() as day (day)}
										{@const entry = editTimetableEntryFor(day, row)}
										{@const observedDate = dateForEditTimetableDay(day)}
										<Table.Cell class="align-top">
											{#if entry}
												<Button
													type="button"
													variant={selectedEditTimetableEntryId === entry.id &&
													selectedEditTimetableDate === observedDate
														? 'default'
														: 'outline'}
													class="h-auto w-full justify-start whitespace-normal px-3 py-2 text-left"
													disabled={!editDateInCycle(observedDate)}
													onclick={() => selectLessonTimetableEntry(entry, observedDate)}
												>
													<div class="space-y-1">
														<p class="font-medium">{editTimetableEntryTitle(entry)}</p>
														<p class="text-xs opacity-80">{editTimetableEntryMeta(entry)}</p>
													</div>
												</Button>
											{:else}
												<div
													class="rounded-md border border-dashed p-3 text-xs text-muted-foreground"
												>
													ว่าง
												</div>
											{/if}
										</Table.Cell>
									{/each}
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				</div>
			{/if}

			<div class="grid gap-4 rounded-md border p-3 sm:grid-cols-2">
				<div class="space-y-2 sm:col-span-2">
					<Label>คาบกำหนดเอง</Label>
					<p class="text-xs text-muted-foreground">
						แก้ช่องด้านล่างเฉพาะกรณีไม่ได้เลือกคาบจากตารางสอน
					</p>
				</div>
				<div class="space-y-2">
					<Label>วิชา</Label>
					<Input bind:value={lessonForm.subjectName} />
				</div>
				<div class="space-y-2">
					<Label>คาบ</Label>
					<Input bind:value={lessonForm.periodLabel} />
				</div>
				<div class="space-y-2">
					<Label>ชั้น/ห้อง</Label>
					<Input bind:value={lessonForm.classroomLabel} />
				</div>
				<div class="space-y-2">
					<Label>ห้องเรียน</Label>
					<Input bind:value={lessonForm.roomLabel} />
				</div>
				<div class="space-y-2">
					<Label>วันที่</Label>
					<Input
						type="date"
						bind:value={lessonForm.observedDate}
						oninput={() => {
							selectedEditTimetableEntryId = '';
							selectedEditTimetableDate = '';
						}}
					/>
				</div>
				<div class="space-y-2">
					<Label>เวลา</Label>
					<Input
						type="time"
						bind:value={lessonForm.observedTime}
						oninput={() => {
							selectedEditTimetableEntryId = '';
							selectedEditTimetableDate = '';
						}}
					/>
				</div>
				<div class="space-y-2 sm:col-span-2">
					<Label>เหตุผล/หมายเหตุ</Label>
					<Textarea
						bind:value={lessonForm.reason}
						rows={3}
						oninput={() => {
							selectedEditTimetableEntryId = '';
							selectedEditTimetableDate = '';
						}}
					/>
				</div>
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (editLessonOpen = false)}>ยกเลิก</Button>
			<LoadingButton
				onclick={saveLessonEdit}
				loading={savingAction === 'lesson'}
				loadingLabel="กำลังบันทึก..."
			>
				บันทึกคาบนิเทศ
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={editEvaluatorsOpen}>
	<Dialog.Content class="sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>แก้ผู้ประเมิน</Dialog.Title>
			<Dialog.Description>
				ผู้ประเมินที่ส่งผลแล้วจะถูกเก็บไว้เพื่อรักษาประวัติและผลประเมิน
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-3">
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
							aria-expanded={evaluatorPickerOpen}
							class="w-full justify-between font-normal"
							disabled={loadingEvaluatorAvailability}
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
										{#if staff.title}
											<span class="ml-1 text-xs text-muted-foreground">({staff.title})</span>
										{/if}
									</Command.Item>
								{/each}
							</Command.Group>
						</Command.List>
					</Command.Root>
				</Popover.Content>
			</Popover.Root>
			{#if unavailableEvaluatorCount > 0}
				<p class="text-xs text-muted-foreground">
					ซ่อนผู้ประเมิน {unavailableEvaluatorCount} คนที่มีงานนิเทศชนช่วงนี้
				</p>
			{/if}
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (editEvaluatorsOpen = false)}>ยกเลิก</Button>
			<LoadingButton
				onclick={saveEvaluatorEdit}
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
			<Dialog.Description>
				ระบบจะเปลี่ยนสถานะเป็นยกเลิกเพื่อเก็บประวัติ ไม่ลบข้อมูลออกจากฐานข้อมูล
			</Dialog.Description>
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
				loadingLabel="กำลังยกเลิก..."
			>
				ยืนยันยกเลิก
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
