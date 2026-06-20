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
		cancelRequestedSupervisionObservation,
		cancelSupervisionObservation,
		getSupervisionObservation,
		getSupervisionTemplate,
		listSupervisionCycles,
		replaceSupervisionObservationEvaluators,
		updateRequestedSupervisionObservation,
		updateSupervisionObservation,
		type ManualLesson,
		type SupervisionCycle,
		type SupervisionEvaluator,
		type SupervisionObservation,
		type SupervisionObservationStatus,
		type SupervisionTemplate
	} from '$lib/api/supervision';
	import { lookupStaff, type StaffLookupItem } from '$lib/api/lookup';
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
	let template = $state<SupervisionTemplate | null>(null);
	let cycles = $state<SupervisionCycle[]>([]);
	let staffList = $state<StaffLookupItem[]>([]);
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
		staffList.filter((staff) => selectedEvaluatorIds.includes(staff.id))
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
					evaluators_submitted: 'ผู้ประเมินส่งครบแล้ว',
					under_review: 'รอตรวจทาน',
					returned: 'ส่งกลับแก้ไข',
					approved: 'อนุมัติผลแล้ว',
					published: 'เผยแพร่ผลแล้ว',
					acknowledged: 'รับทราบแล้ว',
					completed: 'เสร็จสิ้น',
					cancelled: 'ยกเลิก'
				} satisfies Record<SupervisionObservationStatus, string>
			)[status] ?? status
		);
	}

	function evaluatorStatusLabel(status: SupervisionEvaluator['status']): string {
		return (
			{
				assigned: 'มอบหมายแล้ว',
				draft: 'บันทึกร่าง',
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
				returned: 'ส่งกลับแก้ไข',
				request_returned: 'ส่งกลับคำขอ',
				evaluator_draft_saved: 'บันทึกร่างผลประเมิน',
				evaluator_submitted: 'ส่งผลประเมิน',
				submitted_for_review: 'ส่งตรวจทาน',
				approved: 'อนุมัติผล',
				result_approved: 'อนุมัติผล',
				result_returned: 'ส่งกลับผล',
				published: 'เผยแพร่ผล',
				result_published: 'เผยแพร่ผล',
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
		try {
			const loadedObservation = await getSupervisionObservation(data.observationId);
			observation = loadedObservation;
			const [cycleItems, loadedTemplate] = await Promise.all([
				listSupervisionCycles(),
				getSupervisionTemplate(loadedObservation.templateId)
			]);
			cycles = cycleItems;
			template = loadedTemplate;
		} catch (loadError) {
			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดรายการนิเทศได้';
			toast.error(error);
		} finally {
			loading = false;
		}
	}

	function openLessonEditor() {
		if (!observation || !canEditLesson) return;
		lessonForm = {
			subjectName: observationSubjectLabel(observation),
			classroomLabel: observationClassroomLabel(observation),
			roomLabel: observationRoomLabel(observation) === '-' ? '' : observationRoomLabel(observation),
			periodLabel: observationPeriodLabel(observation),
			observedDate: formatDateInput(observation.observedAt),
			observedTime: formatTimeInput(observation.observedAt),
			reason: observation.manualLesson?.reason ?? 'แก้ไขรายการนิเทศจากหน้ารายละเอียด'
		};
		editLessonOpen = true;
	}

	async function saveLessonEdit() {
		if (!observation || !canEditLesson) return;
		if (
			!lessonForm.subjectName ||
			!lessonForm.classroomLabel ||
			!lessonForm.periodLabel ||
			!lessonForm.observedDate
		) {
			toast.error('กรอกวิชา ชั้น/ห้อง คาบ และวันที่ให้ครบ');
			return;
		}
		const manualLesson: ManualLesson = {
			subjectName: lessonForm.subjectName,
			classroomLabel: lessonForm.classroomLabel,
			roomLabel: lessonForm.roomLabel || null,
			periodLabel: lessonForm.periodLabel,
			observedAt: toIsoDateTime(lessonForm.observedDate, lessonForm.observedTime),
			reason: lessonForm.reason || 'แก้ไขรายการนิเทศ'
		};

		savingAction = 'lesson';
		try {
			const response = canManageObservation
				? await updateSupervisionObservation(observation.id, { manualLesson })
				: await updateRequestedSupervisionObservation(observation.id, { manualLesson });
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
		if (staffList.length === 0) {
			staffList = await lookupStaff({ limit: 5000 });
		}
		selectedEvaluatorIds = observation.evaluators.map((evaluator) => evaluator.evaluatorUserId);
		editEvaluatorsOpen = true;
	}

	function toggleEvaluator(staff: StaffLookupItem) {
		if (!canEditEvaluators) return;
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
									<Button variant="outline" onclick={openLessonEditor}>
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
						<Card.Title>แบบประเมินและผล</Card.Title>
						<Card.Description>ข้อมูลแบบประเมิน คะแนนเฉลี่ย และสถานะผลประเมิน</Card.Description>
					</Card.Header>
					<Card.Content class="grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-4">
						<div class="rounded-md border bg-muted/20 p-3">
							<p class="text-xs text-muted-foreground">แบบประเมิน</p>
							<p class="font-medium">{template?.title ?? 'ไม่พบแบบประเมิน'}</p>
						</div>
						<div class="rounded-md border bg-muted/20 p-3">
							<p class="text-xs text-muted-foreground">คะแนนเฉลี่ย</p>
							<p class="font-medium">
								{observation.averageRating === null || observation.averageRating === undefined
									? '-'
									: observation.averageRating.toFixed(2)}
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
	<Dialog.Content class="sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>แก้คาบ/วันเวลา</Dialog.Title>
			<Dialog.Description>
				การแก้จากหน้ารายละเอียดจะบันทึกเป็นคาบกำหนดเอง เพื่อเก็บรายละเอียดที่แก้ไว้ชัดเจน
			</Dialog.Description>
		</Dialog.Header>
		<div class="grid gap-4 sm:grid-cols-2">
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
				<Input type="date" bind:value={lessonForm.observedDate} />
			</div>
			<div class="space-y-2">
				<Label>เวลา</Label>
				<Input type="time" bind:value={lessonForm.observedTime} />
			</div>
			<div class="space-y-2 sm:col-span-2">
				<Label>เหตุผล/หมายเหตุ</Label>
				<Textarea bind:value={lessonForm.reason} rows={3} />
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
