<script lang="ts">
	import { onMount } from 'svelte';
	import { ArrowRight, CheckCircle2, History, ShieldAlert } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { registerAcademicContextDirtySource } from '$lib/academic-context/store';
	import {
		getPromotionRunImpacts,
		resolvePromotionRunImpact,
		type PromotionCorrectionImpact,
		type PromotionImpactWorkspace,
		type PromotionPolicyOptions,
		type PromotionRunStudent,
		type ResolvePromotionImpactInput
	} from '$lib/api/academic-promotion';
	import { ApiClientError } from '$lib/api/client';
	import { lookupHomerooms, type HomeroomLookupItem } from '$lib/api/lookup';
	import {
		decisionError,
		decisionUsesDestination,
		initialDecision,
		outcomeLabels,
		type DecisionDraft
	} from '$lib/academic/lifecycle/promotion-presentation';
	import { formatEffectiveResultValue } from '$lib/academic/results/presentation';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { LoadingButton, PageSkeleton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Textarea } from '$lib/components/ui/textarea';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let {
		runId,
		students,
		options,
		disabled = false
	}: {
		runId: string;
		students: PromotionRunStudent[];
		options: PromotionPolicyOptions | null;
		disabled?: boolean;
	} = $props();
	const latest = new LatestRequest();
	const dates = new Intl.DateTimeFormat('th-TH', {
		dateStyle: 'medium',
		timeStyle: 'short',
		timeZone: 'Asia/Bangkok'
	});
	let alive = false;
	let open = $state(false);
	let loading = $state(false);
	let errorMessage = $state('');
	let needsRefresh = $state(false);
	let workspace = $state.raw<PromotionImpactWorkspace | null>(null);
	let resolutionImpactId = $state<string | null>(null);
	let resolutionKind = $state<'keep_existing' | 'replace_decision'>('keep_existing');
	let resolutionReason = $state('');
	let resolutionError = $state('');
	let resolutionNeedsRefresh = $state(false);
	let resolutionInput = $state.raw<ResolvePromotionImpactInput | null>(null);
	let saving = $state(false);
	let rooms = $state.raw<HomeroomLookupItem[]>([]);
	let roomsLoading = $state(false);
	let roomSearch = $state('');
	let roomRevision = 0;
	let draft = $state<DecisionDraft>(emptyDecision());
	const read = $derived($can.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL));
	const correct = $derived(read && $can.has(PERMISSIONS.ACADEMIC_PROMOTION_CORRECT_SCHOOL));
	const studentNames = $derived(new Map(students.map((row) => [row.item.id, row.studentName])));
	const selectedImpact = $derived(
		workspace?.impacts.find((impact) => impact.id === resolutionImpactId) ?? null
	);
	const selectedStudent = $derived(
		selectedImpact ? (students.find((row) => row.item.id === selectedImpact.itemId) ?? null) : null
	);
	const eligibleRooms = $derived(
		rooms.filter((room) => room.gradeLevelId === draft.targetGradeLevelId)
	);
	const resolutionValidation = $derived.by(() => {
		if (!selectedImpact || !selectedStudent) return 'ไม่พบรายการที่ต้องจัดการ';
		const reason = resolutionReason.trim();
		if (!reason) return 'ระบุเหตุผลการจัดการ';
		if (Array.from(reason).length > 1000) return 'เหตุผลยาวได้ไม่เกิน 1000 ตัวอักษร';
		let digits = 0;
		for (const character of reason) {
			if (/\p{N}/u.test(character)) {
				if (++digits >= 13) return 'ไม่ใส่เลขประจำตัวประชาชนในเหตุผล';
			} else if (/\p{L}/u.test(character)) digits = 0;
		}
		if (resolutionKind === 'keep_existing') return '';
		return decisionError({ ...draft, reason }, selectedStudent.item);
	});

	function emptyDecision(): DecisionDraft {
		return {
			outcome: '',
			targetGradeLevelId: null,
			targetStudyProgramId: null,
			targetHomeroomId: null,
			reason: null,
			condition: null
		};
	}
	function gradeName(id: string | null | undefined) {
		const grade = options?.grades.find((row) => row.id === id);
		if (!grade) return 'ยังไม่ระบุชั้น';
		const labels: Record<string, string> = { kindergarten: 'อ.', primary: 'ป.', secondary: 'ม.' };
		return `${labels[grade.levelType] ?? grade.levelType}${grade.year}`;
	}
	function programName(id: string | null | undefined) {
		return options?.programs.find((program) => program.id === id)?.name ?? 'ยังไม่ระบุแผน';
	}
	function message(cause: unknown) {
		return cause instanceof Error ? cause.message : 'ดำเนินการไม่สำเร็จ กรุณาลองใหม่';
	}
	function invalidateResolutionRequest() {
		resolutionInput = null;
		resolutionNeedsRefresh = false;
		resolutionError = '';
	}

	async function load(more = false) {
		if (!read || !open || loading || (more && (needsRefresh || !workspace?.nextCursor))) return;
		const { revision, signal } = latest.begin();
		const previous = more ? workspace : null;
		loading = true;
		errorMessage = '';
		try {
			const data = await getPromotionRunImpacts(
				runId,
				previous?.nextCursor ? { afterId: previous.nextCursor } : {},
				{ signal }
			);
			if (!alive || !latest.isCurrent(revision) || !read) return;
			if (previous && previous.sourceChecksum !== data.sourceChecksum) {
				needsRefresh = true;
				errorMessage = 'ข้อมูลเปลี่ยนระหว่างตรวจสอบ กรุณาโหลดข้อมูลล่าสุดก่อนดูหน้าถัดไป';
				return;
			}
			workspace = previous ? { ...data, impacts: [...previous.impacts, ...data.impacts] } : data;
			needsRefresh = false;
		} catch (error) {
			if (alive && latest.isCurrent(revision) && !isAbortError(error)) {
				errorMessage = message(error);
				needsRefresh = true;
			}
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}
	function changeOpen(value: boolean) {
		if (saving) return;
		open = value;
		if (value) {
			workspace = null;
			needsRefresh = false;
			void load();
		} else {
			latest.abort();
			loading = false;
			closeResolution();
		}
	}
	function openResolution(impact: PromotionCorrectionImpact) {
		if (!correct || impact.resolution) return;
		const student = students.find((row) => row.item.id === impact.itemId);
		if (!student) return;
		resolutionImpactId = impact.id;
		resolutionKind = 'keep_existing';
		resolutionReason = '';
		draft = initialDecision(student.item);
		rooms = [];
		roomSearch = '';
		invalidateResolutionRequest();
	}
	function closeResolution() {
		if (saving) return;
		resolutionImpactId = null;
		resolutionReason = '';
		draft = emptyDecision();
		rooms = [];
		roomRevision++;
		invalidateResolutionRequest();
	}
	function changeResolutionKind(value: string) {
		if (value !== 'keep_existing' && value !== 'replace_decision') return;
		resolutionKind = value;
		invalidateResolutionRequest();
	}
	function changeOutcome(value: string) {
		if (!Object.hasOwn(outcomeLabels, value)) return;
		draft.outcome = value as keyof typeof outcomeLabels;
		if (!decisionUsesDestination(draft.outcome)) {
			draft.targetGradeLevelId = null;
			draft.targetStudyProgramId = null;
			draft.targetHomeroomId = null;
		} else if (draft.outcome === 'repeat' && selectedStudent) {
			draft.targetGradeLevelId = selectedStudent.item.sourceGradeLevelId;
		}
		if (draft.outcome !== 'conditional') draft.condition = null;
		invalidateResolutionRequest();
	}
	async function loadRooms() {
		if (!workspace || !correct || saving) return;
		const revision = ++roomRevision;
		roomsLoading = true;
		try {
			const result = await lookupHomerooms({
				academicYearId: workspace.targetYearId,
				activeOnly: true,
				limit: 500,
				search: roomSearch
			});
			if (alive && revision === roomRevision) rooms = result;
		} catch (cause) {
			if (alive && revision === roomRevision) resolutionError = message(cause);
		} finally {
			if (alive && revision === roomRevision) roomsLoading = false;
		}
	}
	async function reloadResolutionEvidence() {
		if (saving) return;
		await load();
		if (!alive) return;
		resolutionInput = null;
		resolutionNeedsRefresh = false;
		resolutionError = '';
		if (!selectedImpact || selectedImpact.resolution) closeResolution();
	}
	async function saveResolution() {
		if (
			!workspace ||
			!selectedImpact ||
			!selectedStudent ||
			!correct ||
			saving ||
			resolutionNeedsRefresh ||
			resolutionValidation
		)
			return;
		const reason = resolutionReason.trim();
		resolutionInput ??= {
			requestId: crypto.randomUUID(),
			sourceChecksum: workspace.sourceChecksum,
			resolutionKind,
			replacementDecision:
				resolutionKind === 'replace_decision'
					? { ...draft, outcome: draft.outcome as keyof typeof outcomeLabels, reason }
					: null,
			reason
		};
		saving = true;
		resolutionError = '';
		try {
			await resolvePromotionRunImpact(runId, selectedImpact.id, resolutionInput);
			if (!alive) return;
			resolutionInput = null;
			resolutionImpactId = null;
			toast.success('บันทึกการจัดการผลกระทบแล้ว');
			await load();
		} catch (cause) {
			if (!alive) return;
			resolutionError = message(cause);
			if (cause instanceof ApiClientError && cause.status === 409) {
				resolutionNeedsRefresh = true;
				resolutionInput = null;
			}
		} finally {
			if (alive) saving = false;
		}
	}

	onMount(() => {
		alive = true;
		const unregister = registerAcademicContextDirtySource(
			`promotion-impact-${runId}`,
			() => !!resolutionImpactId || saving
		);
		const unsubscribe = can.subscribe((permissions) => {
			if (!permissions.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL)) {
				changeOpen(false);
				workspace = null;
			} else if (!permissions.has(PERMISSIONS.ACADEMIC_PROMOTION_CORRECT_SCHOOL)) {
				closeResolution();
			}
		});
		return () => {
			alive = false;
			latest.abort();
			roomRevision++;
			unregister();
			unsubscribe();
		};
	});
</script>

<Button variant="outline" disabled={disabled || !read} onclick={() => changeOpen(true)}>
	<History class="size-4" />ตรวจผลแก้ไขหลังเลื่อนชั้น
</Button>
<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="max-h-[85dvh] overflow-y-auto sm:max-w-2xl"
		showCloseButton={!saving}
		onInteractOutside={(event) => {
			if (saving) event.preventDefault();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>ผลแก้ไขหลังเลื่อนชั้น</Dialog.Title>
			<Dialog.Description>
				ตรวจผลที่เปลี่ยนจากรุ่นอ้างอิง โดยไม่แก้ประวัติรอบเลื่อนชั้นเดิม
			</Dialog.Description>
		</Dialog.Header>
		<div class="flex items-start gap-3 rounded-xl border border-amber-500/30 bg-amber-500/5 p-3 text-sm">
			<ShieldAlert class="mt-0.5 size-4 shrink-0 text-amber-700 dark:text-amber-400" />
			<p>
				การเปิดดูหน้านี้ไม่เปลี่ยนชั้นหรือห้อง หากเลือกปรับผลใหม่ ระบบจะปรับได้เฉพาะก่อนเปิดปีการศึกษาปลายทาง
			</p>
		</div>
		{#if loading && !workspace}
			<PageSkeleton variant="form" rows={3} />
		{:else if workspace}
			<div class="flex flex-wrap items-center justify-between gap-2">
				<p class="text-sm text-muted-foreground">
					แสดง {workspace.impacts.length} จาก {workspace.totalCount} รายการแก้ไข
				</p>
				<Badge variant={workspace.pendingCount ? 'destructive' : 'secondary'}>
					รอจัดการ {workspace.pendingCount} รายการ
				</Badge>
			</div>
			{#if workspace.impacts.length === 0}
				<div class="rounded-lg border border-dashed p-6 text-center">
					<p class="font-medium">ยังไม่พบผลแก้ไขหลังเลื่อนชั้น</p>
					<p class="mt-1 text-sm text-muted-foreground">
						เทียบกับผลรายปีที่ใช้ในรายการซึ่งดำเนินการแล้ว
					</p>
				</div>
			{:else}
				<ul class="divide-y rounded-xl border bg-card">
					{#each workspace.impacts as impact (impact.id)}
						<li class="space-y-3 p-3 sm:p-4">
							<div class="flex flex-wrap justify-between gap-x-3 gap-y-1">
								<div>
									<p class="font-semibold">
										{studentNames.get(impact.itemId) ?? 'นักเรียนในรอบนี้'}
									</p>
									<p class="text-sm">
										{impact.evidence.offeringCode} · {impact.evidence.offeringName}
									</p>
								</div>
								<div class="text-right">
									{#if impact.resolution}
										<Badge variant="secondary">
											<CheckCircle2 class="size-3.5" />จัดการแล้ว · {impact.resolution
												.resolutionKind === 'keep_existing'
												? 'คงผลเดิม'
												: 'ปรับผลใหม่'}
										</Badge>
									{:else}
										<Badge variant="destructive">ยังไม่ได้จัดการ</Badge>
									{/if}
									<p class="mt-1 text-xs text-muted-foreground">{impact.evidence.termName}</p>
								</div>
							</div>
							{#if impact.evidence.evaluationDomain}
								<p class="text-xs text-muted-foreground">
									{impact.evidence.evaluationDomain === 'desirable_characteristic'
										? 'คุณลักษณะอันพึงประสงค์'
										: 'การอ่าน คิดวิเคราะห์ และเขียน'} · {impact.evidence.criterionName}
								</p>
							{/if}
							<div class="flex flex-wrap items-center justify-between gap-2">
								<div class="flex items-center gap-2 text-sm">
									<span class="text-muted-foreground">แก้จาก</span>
									<span aria-label="ผลก่อนแก้" class="font-medium">
										{formatEffectiveResultValue(impact.evidence.correction.previous)}
									</span>
									<ArrowRight class="size-4 text-muted-foreground" />
									<span aria-label="ผลหลังแก้" class="font-semibold text-primary">
										{formatEffectiveResultValue(impact.evidence.correction.corrected)}
									</span>
								</div>
								<time
									class="text-xs text-muted-foreground"
									datetime={impact.evidence.correction.correctedAt}
								>
									{dates.format(new Date(impact.evidence.correction.correctedAt))}
								</time>
							</div>
							{#if impact.resolution}
								<div class="rounded-lg bg-muted/50 p-3 text-sm">
									<p>{impact.resolution.reason}</p>
									<time
										class="mt-1 block text-xs text-muted-foreground"
										datetime={impact.resolution.resolvedAt}
									>
										บันทึกเมื่อ {dates.format(new Date(impact.resolution.resolvedAt))}
									</time>
								</div>
							{:else if correct}
								<div class="flex justify-end">
									<Button
										size="sm"
										disabled={disabled || loading}
										onclick={() => openResolution(impact)}
									>
										จัดการผลกระทบ
									</Button>
								</div>
							{:else}
								<p class="text-xs text-muted-foreground">
									รอผู้มีสิทธิ์แก้ผลเลื่อนชั้นตรวจสอบรายการนี้
								</p>
							{/if}
						</li>
					{/each}
				</ul>
				{#if workspace.nextCursor}
					<LoadingButton
						variant="outline"
						{loading}
						disabled={needsRefresh}
						onclick={() => void load(true)}
					>
						แสดงเพิ่มเติม
					</LoadingButton>
				{/if}
			{/if}
		{/if}
		{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		<Dialog.Footer>
			<LoadingButton variant="outline" {loading} onclick={() => void load()}>
				โหลดข้อมูลล่าสุด
			</LoadingButton>
			<Button onclick={() => changeOpen(false)}>กลับไปรอบเลื่อนชั้น</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root
	open={!!resolutionImpactId}
	onOpenChange={(value) => {
		if (!value) closeResolution();
	}}
>
	<Dialog.Content
		class="max-h-[90dvh] overflow-y-auto sm:max-w-xl"
		showCloseButton={!saving}
		onInteractOutside={(event) => {
			if (saving) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (saving) event.preventDefault();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>จัดการผลกระทบหลังแก้ผล</Dialog.Title>
			<Dialog.Description>
				{selectedStudent?.studentName ?? 'นักเรียนในรอบนี้'} · {selectedImpact?.evidence
					.offeringCode ?? ''}
			</Dialog.Description>
		</Dialog.Header>
		{#if selectedImpact && selectedStudent}
			<div class="rounded-xl border bg-muted/30 p-3 text-sm">
				<p class="font-medium">
					ผลที่แก้: {formatEffectiveResultValue(selectedImpact.evidence.correction.previous)} → {formatEffectiveResultValue(selectedImpact.evidence.correction.corrected)}
				</p>
				<p class="mt-1 text-xs text-muted-foreground">
					การปรับผลใหม่จะสร้างหลักฐานเพิ่ม และไม่แก้รอบเลื่อนชั้นหรือหลักฐานเดิม
				</p>
			</div>
			<div class="space-y-2">
				<Label for="impact-resolution-kind">วิธีจัดการ</Label>
				<Select.Root
					type="single"
					value={resolutionKind}
					onValueChange={changeResolutionKind}
					disabled={saving || resolutionNeedsRefresh}
				>
					<Select.Trigger id="impact-resolution-kind" class="w-full">
						{resolutionKind === 'keep_existing' ? 'คงผลเลื่อนชั้นเดิม' : 'ปรับผลเลื่อนชั้นใหม่'}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="keep_existing">คงผลเลื่อนชั้นเดิม</Select.Item>
						<Select.Item value="replace_decision">ปรับผลเลื่อนชั้นใหม่</Select.Item>
					</Select.Content>
				</Select.Root>
			</div>
			{#if resolutionKind === 'replace_decision'}
				<div class="space-y-2">
					<Label for="impact-outcome">ผลเลื่อนชั้นใหม่</Label>
					<Select.Root
						type="single"
						value={draft.outcome}
						onValueChange={changeOutcome}
						disabled={saving || resolutionNeedsRefresh}
					>
						<Select.Trigger id="impact-outcome" class="w-full">
							{draft.outcome ? outcomeLabels[draft.outcome] : 'เลือกผลเลื่อนชั้น'}
						</Select.Trigger>
						<Select.Content>
							{#each Object.entries(outcomeLabels) as [value, label] (value)}
								<Select.Item {value}>{label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				{#if decisionUsesDestination(draft.outcome)}
					<div class="space-y-2">
						<Label for="impact-grade">ชั้นปลายทาง</Label>
						<Select.Root
							type="single"
							value={draft.targetGradeLevelId ?? ''}
							onValueChange={(value) => {
								draft.targetGradeLevelId = value || null;
								draft.targetHomeroomId = null;
								invalidateResolutionRequest();
							}}
							disabled={saving || resolutionNeedsRefresh || draft.outcome === 'repeat'}
						>
							<Select.Trigger id="impact-grade" class="w-full">
								{gradeName(draft.targetGradeLevelId)}
							</Select.Trigger>
							<Select.Content>
								{#each options?.grades.filter((grade) => grade.isActive) ?? [] as grade (grade.id)}
									<Select.Item value={grade.id}>{gradeName(grade.id)}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label for="impact-program">แผนการเรียนปลายทาง</Label>
						<Select.Root
							type="single"
							value={draft.targetStudyProgramId ?? ''}
							onValueChange={(value) => {
								draft.targetStudyProgramId = value || null;
								draft.targetHomeroomId = null;
								invalidateResolutionRequest();
							}}
							disabled={saving || resolutionNeedsRefresh}
						>
							<Select.Trigger id="impact-program" class="w-full">
								{programName(draft.targetStudyProgramId)}
							</Select.Trigger>
							<Select.Content>
								{#each options?.programs.filter((program) => program.status === 'published') ?? [] as program (program.id)}
									<Select.Item value={program.id}>{program.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label for="impact-room">ห้องปลายทาง (จัดภายหลังได้)</Label>
						<div class="flex gap-2">
							<Input
								aria-label="ค้นหาห้องปลายทางสำหรับผลใหม่"
								bind:value={roomSearch}
								disabled={saving || resolutionNeedsRefresh}
								placeholder="ค้นหาชื่อห้อง"
							/>
							<LoadingButton
								variant="outline"
								loading={roomsLoading}
								disabled={saving || resolutionNeedsRefresh}
								onclick={loadRooms}
							>
								ค้นหาห้อง
							</LoadingButton>
						</div>
						<Select.Root
							type="single"
							value={draft.targetHomeroomId ?? 'later'}
							onValueChange={(value) => {
								draft.targetHomeroomId = value === 'later' ? null : value;
								invalidateResolutionRequest();
							}}
							disabled={saving || resolutionNeedsRefresh}
						>
							<Select.Trigger id="impact-room" class="w-full">
								{draft.targetHomeroomId
									? (rooms.find((room) => room.id === draft.targetHomeroomId)?.name ??
										'ห้องที่บันทึกไว้')
									: 'จัดห้องภายหลัง'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="later">จัดห้องภายหลัง</Select.Item>
								{#each eligibleRooms as room (room.id)}
									<Select.Item value={room.id}>{room.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
				{/if}
				{#if draft.outcome === 'conditional'}
					<div class="space-y-2">
						<Label for="impact-condition">เงื่อนไขที่ต้องติดตาม</Label>
						<Textarea
							id="impact-condition"
							value={draft.condition ?? ''}
							oninput={(event) => {
								draft.condition = event.currentTarget.value || null;
								invalidateResolutionRequest();
							}}
							disabled={saving || resolutionNeedsRefresh}
							maxlength={1000}
						/>
					</div>
				{/if}
			{/if}
			<div class="space-y-2">
				<Label for="impact-resolution-reason">เหตุผลการจัดการ</Label>
				<Textarea
					id="impact-resolution-reason"
					value={resolutionReason}
					oninput={(event) => {
						resolutionReason = event.currentTarget.value;
						invalidateResolutionRequest();
					}}
					disabled={saving || resolutionNeedsRefresh}
					maxlength={1000}
				/>
			</div>
			{#if resolutionValidation && !resolutionNeedsRefresh}
				<p class="text-sm text-amber-700 dark:text-amber-400">{resolutionValidation}</p>
			{/if}
			{#if resolutionError}<p role="alert" class="text-sm text-destructive">{resolutionError}</p>{/if}
		{/if}
		<Dialog.Footer>
			<Button variant="outline" disabled={saving} onclick={closeResolution}>ยกเลิก</Button>
			{#if resolutionNeedsRefresh}
				<LoadingButton loading={loading} onclick={reloadResolutionEvidence}>
					โหลดหลักฐานล่าสุด
				</LoadingButton>
			{:else}
				<LoadingButton
					loading={saving}
					disabled={!!resolutionValidation}
					onclick={saveResolution}
				>
					ยืนยันการจัดการ
				</LoadingButton>
			{/if}
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
