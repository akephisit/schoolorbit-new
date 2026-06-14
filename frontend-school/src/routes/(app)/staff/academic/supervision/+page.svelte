<script lang="ts">
	import { onMount } from 'svelte';
	import {
		BarChart3,
		BookOpenCheck,
		ClipboardCheck,
		FileSignature,
		RefreshCw,
		Send,
		Settings2,
		UserCheck
	} from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Tabs from '$lib/components/ui/tabs';
	import { Textarea } from '$lib/components/ui/textarea';
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
		type CreateSupervisionCycleRequest,
		type CreateSupervisionTemplateRequest,
		type SaveEvaluationRequest,
		type SupervisionCycle,
		type SupervisionCycleProgress,
		type SupervisionObservation,
		type SupervisionObservationStatus,
		type SupervisionTemplate
	} from '$lib/api/supervision';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';

	type ResponseDraft = {
		ratingScore: string;
		textResponse: string;
	};

	let { data } = $props();

	let loading = $state(true);
	let saving = $state(false);
	let activeTab = $state('mine');
	let cycles = $state<SupervisionCycle[]>([]);
	let templates = $state<SupervisionTemplate[]>([]);
	let observations = $state<SupervisionObservation[]>([]);
	let timetableEntries = $state<TimetableEntry[]>([]);
	let staffList = $state<StaffLookupItem[]>([]);
	let selectedCycleId = $state('');
	let selectedTimetableEntryId = $state('');
	let manualMode = $state(false);
	let manualLesson = $state({
		subjectName: '',
		classroomLabel: '',
		roomLabel: '',
		observedAt: '',
		periodLabel: '',
		reason: ''
	});
	let requestReturnComment = $state('');
	let approvalObservationId = $state('');
	let approvalEvaluatorId = $state('');
	let evaluationObservationId = $state('');
	let responseDrafts = $state<{ [itemId: string]: ResponseDraft }>({});
	let acknowledgeComment = $state('');
	let reviewComment = $state('');
	let progressCycleId = $state('');
	let progress = $state<SupervisionCycleProgress | null>(null);
	let cycleForm = $state({
		academicYear: new Date().getFullYear() + 543,
		semester: '1',
		title: '',
		description: '',
		templateId: '',
		bookingOpensAt: '',
		bookingClosesAt: '',
		startsAt: '',
		endsAt: ''
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
	const canManage = $derived($can.has(PERMISSIONS.SUPERVISION_MANAGE_SCHOOL));
	const canEvaluate = $derived($can.has(PERMISSIONS.SUPERVISION_EVALUATE_ASSIGNED));
	const canApprove = $derived($can.has(PERMISSIONS.SUPERVISION_APPROVE_SCHOOL));
	const canReport = $derived(
		$can.has(PERMISSIONS.SUPERVISION_READ_SCHOOL) || canManage || canApprove
	);
	const openCycles = $derived(cycles.filter((cycle) => cycle.status === 'open'));
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
	const selectedEvaluation = $derived(
		observations.find((observation) => observation.id === evaluationObservationId) ?? null
	);
	const selectedEvaluationTemplate = $derived(
		selectedEvaluation
			? (templates.find((template) => template.id === selectedEvaluation.templateId) ?? null)
			: null
	);

	function formatDate(value?: string | null): string {
		if (!value) return '-';
		return new Intl.DateTimeFormat('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short'
		}).format(new Date(value));
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
		return `${entry.day_of_week}${period} - ${title}${room}`;
	}

	function toIsoDateTime(value: string): string {
		return new Date(value).toISOString();
	}

	async function refreshAll() {
		loading = true;
		try {
			const [cycleItems, templateItems, observationItems, timetable, staffItems] =
				await Promise.all([
					listSupervisionCycles(),
					listSupervisionTemplates(),
					listSupervisionObservations(),
					getMyTimetable({ include_team_ghosts: true }),
					lookupStaff({ activeOnly: true, limit: 1000 })
				]);
			cycles = cycleItems;
			templates = templateItems;
			observations = observationItems;
			timetableEntries = timetable.data;
			staffList = staffItems;
			selectedCycleId ||= openCycles[0]?.id ?? cycles[0]?.id ?? '';
			progressCycleId ||= cycles[0]?.id ?? '';
			cycleForm.templateId ||= templates[0]?.id ?? '';
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถโหลดข้อมูลนิเทศได้');
		} finally {
			loading = false;
		}
	}

	async function createBookingRequest() {
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
			(!manualLesson.subjectName || !manualLesson.classroomLabel || !manualLesson.observedAt)
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
							observedAt: toIsoDateTime(manualLesson.observedAt),
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

	async function approveRequest() {
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
		if (!cycleForm.title || !cycleForm.templateId || !cycleForm.startsAt || !cycleForm.endsAt) {
			toast.error('กรอกชื่อรอบ แบบประเมิน และช่วงวันที่ให้ครบ');
			return;
		}

		const payload: CreateSupervisionCycleRequest = {
			academicYear: Number(cycleForm.academicYear),
			semester: cycleForm.semester,
			title: cycleForm.title,
			description: cycleForm.description || null,
			templateId: cycleForm.templateId,
			bookingOpensAt: cycleForm.bookingOpensAt ? toIsoDateTime(cycleForm.bookingOpensAt) : null,
			bookingClosesAt: cycleForm.bookingClosesAt ? toIsoDateTime(cycleForm.bookingClosesAt) : null,
			startsAt: toIsoDateTime(cycleForm.startsAt),
			endsAt: toIsoDateTime(cycleForm.endsAt),
			status: 'draft',
			targets: [{ targetType: 'school', requiredObservations: 1, priority: 100 }]
		};

		saving = true;
		try {
			const response = await createSupervisionCycle(payload);
			if (!response.success) throw new Error(response.error || 'สร้างรอบนิเทศไม่สำเร็จ');
			toast.success('สร้างรอบนิเทศแล้ว');
			cycleForm.title = '';
			cycleForm.description = '';
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'สร้างรอบนิเทศไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function createTemplate() {
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
			await refreshAll();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'สร้างแบบประเมินไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function loadProgress() {
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

<section class="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6">
	<header class="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
		<div class="space-y-1">
			<div class="flex items-center gap-2">
				<ClipboardCheck class="h-7 w-7 text-primary" />
				<h1 class="text-2xl font-bold text-foreground">นิเทศการสอน</h1>
			</div>
			<p class="text-sm text-muted-foreground">
				จัดรอบนิเทศ จองคาบ ประเมิน ส่งตรวจทาน และรับทราบผลในพื้นที่เดียว
			</p>
		</div>

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
	</header>

	<div class="flex flex-wrap gap-2">
		<Button variant="outline" size="sm" onclick={refreshAll} disabled={loading || saving}>
			<RefreshCw class="mr-2 h-4 w-4" />
			รีเฟรช
		</Button>
	</div>

	<Tabs.Root bind:value={activeTab} class="space-y-4">
		<Tabs.List class="grid w-full grid-cols-2 md:grid-cols-6">
			<Tabs.Trigger value="mine">ของฉัน</Tabs.Trigger>
			<Tabs.Trigger value="requests" disabled={!canManage}>คำขอจอง</Tabs.Trigger>
			<Tabs.Trigger value="evaluate" disabled={!canEvaluate}>ประเมิน</Tabs.Trigger>
			<Tabs.Trigger value="cycles" disabled={!canManage}>รอบนิเทศ</Tabs.Trigger>
			<Tabs.Trigger value="templates" disabled={!canManage}>แบบประเมิน</Tabs.Trigger>
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
						<p class="text-sm text-muted-foreground">บัญชีนี้ยังไม่มีสิทธิ์จองคาบนิเทศของตนเอง</p>
					{:else}
						<div class="grid gap-4 lg:grid-cols-2">
							<div class="space-y-2">
								<Label>รอบนิเทศ</Label>
								<select
									class="h-9 rounded-md border bg-background px-3 text-sm"
									bind:value={selectedCycleId}
								>
									<option value="">เลือกรอบนิเทศ</option>
									{#each openCycles as cycle (cycle.id)}
										<option value={cycle.id}>{cycle.title}</option>
									{/each}
								</select>
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
								<select
									class="h-9 w-full rounded-md border bg-background px-3 text-sm"
									bind:value={selectedTimetableEntryId}
								>
									<option value="">เลือกคาบสอน</option>
									{#each timetableEntries as entry (entry.id)}
										<option value={entry.id}>{timetableLabel(entry)}</option>
									{/each}
								</select>
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
									<Label>วันและเวลา</Label>
									<Input type="datetime-local" bind:value={manualLesson.observedAt} />
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
				<Card.Content class="space-y-3">
					{#if myObservations.length === 0}
						<p class="text-sm text-muted-foreground">ยังไม่มีรายการนิเทศของฉัน</p>
					{:else}
						{#each myObservations as observation (observation.id)}
							<div class="rounded-md border p-3">
								<div class="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
									<div>
										<p class="font-medium">
											{observation.lessonSnapshot.subjectName ??
												observation.manualLesson?.subjectName ??
												'คาบนิเทศ'}
										</p>
										<p class="text-sm text-muted-foreground">
											{formatDate(
												observation.lessonSnapshot.observedAt ??
													observation.manualLesson?.observedAt
											)}
										</p>
									</div>
									<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
								</div>
								{#if observation.status === 'published'}
									<div class="mt-3 space-y-2">
										<Textarea
											bind:value={acknowledgeComment}
											rows={2}
											placeholder="ความคิดเห็นเพิ่มเติม (ถ้ามี)"
										/>
										<Button
											size="sm"
											onclick={() => acknowledgeResult(observation.id)}
											disabled={saving}
										>
											<FileSignature class="mr-2 h-4 w-4" />
											รับทราบผล
										</Button>
									</div>
								{/if}
							</div>
						{/each}
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
				</Card.Header>
				<Card.Content class="space-y-3">
					{#if requestedObservations.length === 0}
						<p class="text-sm text-muted-foreground">ไม่มีคำขอจองที่รออนุมัติ</p>
					{:else}
						<div class="grid gap-3 lg:grid-cols-[1fr_260px_180px]">
							<select
								class="h-9 rounded-md border bg-background px-3 text-sm"
								bind:value={approvalObservationId}
							>
								<option value="">เลือกรายการคำขอ</option>
								{#each requestedObservations as observation (observation.id)}
									<option value={observation.id}>
										{observation.observedDisplayName ?? 'ครู'} - {observation.lessonSnapshot
											.subjectName ?? 'คาบนิเทศ'}
									</option>
								{/each}
							</select>
							<select
								class="h-9 rounded-md border bg-background px-3 text-sm"
								bind:value={approvalEvaluatorId}
							>
								<option value="">เลือกผู้ประเมิน</option>
								{#each staffList as staff (staff.id)}
									<option value={staff.id}>{staff.name}</option>
								{/each}
							</select>
							<Button onclick={approveRequest} disabled={saving}>อนุมัติและมอบหมาย</Button>
						</div>
						<div class="space-y-2">
							<Label>เหตุผลส่งกลับ</Label>
							<Textarea bind:value={requestReturnComment} rows={2} />
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
						<p class="text-sm text-muted-foreground">ยังไม่มีรายการที่ได้รับมอบหมาย</p>
					{:else}
						<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
							{#each assignedObservations as observation (observation.id)}
								<button
									type="button"
									class="rounded-md border p-3 text-left transition hover:bg-muted/40"
									onclick={() => prepareEvaluationDraft(observation)}
								>
									<div class="font-medium">
										{observation.observedDisplayName ?? 'ครูผู้ถูกนิเทศ'}
									</div>
									<div class="text-sm text-muted-foreground">
										{observation.lessonSnapshot.subjectName ??
											observation.manualLesson?.subjectName ??
											'คาบนิเทศ'}
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
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<Settings2 class="h-5 w-5" />
						สร้างรอบนิเทศ
					</Card.Title>
				</Card.Header>
				<Card.Content class="grid gap-3 lg:grid-cols-2">
					<Input bind:value={cycleForm.title} placeholder="ชื่อรอบนิเทศ" />
					<Input bind:value={cycleForm.academicYear} type="number" placeholder="ปีการศึกษา" />
					<Input bind:value={cycleForm.semester} placeholder="ภาคเรียน" />
					<select
						class="h-9 rounded-md border bg-background px-3 text-sm"
						bind:value={cycleForm.templateId}
					>
						<option value="">เลือกแบบประเมิน</option>
						{#each templates as template (template.id)}
							<option value={template.id}>{template.title}</option>
						{/each}
					</select>
					<Input type="datetime-local" bind:value={cycleForm.bookingOpensAt} />
					<Input type="datetime-local" bind:value={cycleForm.bookingClosesAt} />
					<Input type="datetime-local" bind:value={cycleForm.startsAt} />
					<Input type="datetime-local" bind:value={cycleForm.endsAt} />
					<Textarea
						class="lg:col-span-2"
						bind:value={cycleForm.description}
						rows={2}
						placeholder="รายละเอียด"
					/>
					<div class="lg:col-span-2">
						<Button onclick={createCycle} disabled={saving}>สร้างรอบนิเทศ</Button>
					</div>
				</Card.Content>
			</Card.Root>

			<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
				{#each cycles as cycle (cycle.id)}
					<div class="rounded-md border p-3">
						<div class="flex items-start justify-between gap-3">
							<div>
								<p class="font-medium">{cycle.title}</p>
								<p class="text-sm text-muted-foreground">
									ปี {cycle.academicYear} / {cycle.semester}
								</p>
							</div>
							<Badge variant="secondary">{statusLabel(cycle.status)}</Badge>
						</div>
						<p class="mt-2 text-xs text-muted-foreground">
							{formatDate(cycle.startsAt)} - {formatDate(cycle.endsAt)}
						</p>
					</div>
				{/each}
			</div>
		</Tabs.Content>

		<Tabs.Content value="templates" class="space-y-4">
			<Card.Root>
				<Card.Header>
					<Card.Title>สร้างแบบประเมินพื้นฐาน</Card.Title>
				</Card.Header>
				<Card.Content class="grid gap-3 lg:grid-cols-2">
					<Input bind:value={templateForm.title} placeholder="ชื่อแบบประเมิน" />
					<Input bind:value={templateForm.description} placeholder="รายละเอียด" />
					<Input type="number" bind:value={templateForm.ratingMin} placeholder="คะแนนต่ำสุด" />
					<Input type="number" bind:value={templateForm.ratingMax} placeholder="คะแนนสูงสุด" />
					<Input
						class="lg:col-span-2"
						bind:value={templateForm.ratingLabel}
						placeholder="หัวข้อแบบคะแนน"
					/>
					<Input
						class="lg:col-span-2"
						bind:value={templateForm.textLabel}
						placeholder="หัวข้อแบบข้อความ"
					/>
					<div class="lg:col-span-2">
						<Button onclick={createTemplate} disabled={saving}>สร้างแบบประเมิน</Button>
					</div>
				</Card.Content>
			</Card.Root>

			<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
				{#each templates as template (template.id)}
					<div class="rounded-md border p-3">
						<div class="flex items-start justify-between gap-3">
							<div>
								<p class="font-medium">{template.title}</p>
								<p class="text-sm text-muted-foreground">{template.sections.length} หมวด</p>
							</div>
							<Badge variant="secondary">{template.status}</Badge>
						</div>
					</div>
				{/each}
			</div>
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
						<select
							class="h-9 rounded-md border bg-background px-3 text-sm"
							bind:value={progressCycleId}
						>
							<option value="">เลือกรอบนิเทศ</option>
							{#each cycles as cycle (cycle.id)}
								<option value={cycle.id}>{cycle.title}</option>
							{/each}
						</select>
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
					{/if}

					<div class="space-y-2">
						<Textarea bind:value={reviewComment} rows={2} placeholder="เหตุผลส่งกลับผลนิเทศ" />
						<div class="flex flex-wrap gap-2">
							{#each observations.filter((item) => item.status === 'evaluators_submitted' || item.status === 'under_review' || item.status === 'approved') as observation (observation.id)}
								<div class="flex flex-wrap items-center gap-2 rounded-md border p-2">
									<span class="text-sm">{observation.observedDisplayName ?? 'ครู'}</span>
									<Badge variant="secondary">{statusLabel(observation.status)}</Badge>
									<Button
										size="sm"
										variant="outline"
										onclick={() => submitForReview(observation.id)}
										disabled={saving}
									>
										ส่งตรวจทาน
									</Button>
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
								</div>
							{/each}
						</div>
					</div>
				</Card.Content>
			</Card.Root>
		</Tabs.Content>
	</Tabs.Root>
</section>
