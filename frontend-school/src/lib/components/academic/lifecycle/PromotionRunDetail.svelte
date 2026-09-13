<script lang="ts">
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import { resolve } from '$app/paths';
	import { RefreshCw, ArrowRight } from 'lucide-svelte';
	import { PageShell } from '$lib/components/app-layout';
	import PromotionImpactsDialog from './PromotionImpactsDialog.svelte';
	import { PageState, PageSkeleton, LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Table from '$lib/components/ui/table';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { can } from '$lib/stores/permissions';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { registerAcademicContextDirtySource } from '$lib/academic-context/store';
	import {
		getPromotionRun,
		getPromotionPolicyOptions,
		calculatePromotionRun,
		reviewPromotionItem,
		approvePromotionRun,
		executePromotionRun,
		type PromotionRunWorkspace,
		type PromotionRunStudent,
		type PromotionPolicyOptions,
		type PromotionRunCalculateInput,
		type PromotionRunApproveInput,
		type PromotionRunExecuteInput
	} from '$lib/api/academic-promotion';
	import { lookupHomerooms, type HomeroomLookupItem } from '$lib/api/lookup';
	import {
		initialDecision,
		decisionError,
		decisionUsesDestination,
		canApproveRun,
		canExecuteRun,
		outcomeLabels,
		runStatusLabels,
		findingLabels,
		type DecisionDraft
	} from '$lib/academic/lifecycle/promotion-presentation';

	let { runId }: { runId: string } = $props();
	const requests = new LatestRequest();
	let workspace = $state.raw<PromotionRunWorkspace | null>(null);
	let options = $state.raw<PromotionPolicyOptions | null>(null);
	let rooms = $state.raw<HomeroomLookupItem[]>([]);
	let loading = $state(false);
	let error = $state('');
	let busy = $state<'calculate' | 'approve' | 'execute' | null>(null);
	let confirmation = $state<'calculate' | 'approve' | 'execute' | null>(null);
	let search = $state('');
	let visibleCount = $state(50);
	let selected = $state.raw<PromotionRunStudent | null>(null);
	let draft = $state<DecisionDraft>({
		outcome: '',
		targetGradeLevelId: null,
		targetStudyProgramId: null,
		targetHomeroomId: null,
		reason: null,
		condition: null
	});
	let saving = $state(false);
	let formError = $state('');
	let roomsLoading = $state(false);
	let roomSearch = $state('');
	let roomRevision = 0;
	let stopRequested = $state(false);
	let progress = $state('');
	let alive = true;
	let loaded = false;
	let calculateInput: PromotionRunCalculateInput | null = null;
	let approveInput: PromotionRunApproveInput | null = null;
	let executeInput: PromotionRunExecuteInput | null = null;
	const read = $derived($can.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL));
	const manage = $derived(read && $can.has(PERMISSIONS.ACADEMIC_PROMOTION_MANAGE_SCHOOL));
	const approve = $derived(read && $can.has(PERMISSIONS.ACADEMIC_PROMOTION_APPROVE_SCHOOL));
	const execute = $derived(read && $can.has(PERMISSIONS.ACADEMIC_PROMOTION_EXECUTE_SCHOOL));
	const filtered = $derived(
		workspace?.students.filter((row) =>
			`${row.studentCode ?? ''} ${row.studentName}`
				.toLocaleLowerCase()
				.includes(search.trim().toLocaleLowerCase())
		) ?? []
	);
	const visible = $derived(filtered.slice(0, visibleCount));
	const completedHolds = $derived(
		workspace?.students.filter(
			(row) =>
				(row.receipt || row.item.status === 'executed') && row.item.decision?.outcome === 'hold'
		).length ?? 0
	);
	const validation = $derived(selected ? decisionError(draft, selected.item) : '');
	const editable = $derived(
		!!selected &&
			manage &&
			!!workspace &&
			!['draft', 'executing', 'completed'].includes(workspace.run.status) &&
			selected.item.status !== 'executed' &&
			!selected.needsRecalculation &&
			selected.annualResultCurrent &&
			!busy
	);
	const eligibleRooms = $derived(
		rooms.filter((room) => room.gradeLevelId === draft.targetGradeLevelId)
	);
	function gradeName(id: string | null | undefined) {
		const grade = options?.grades.find((grade) => grade.id === id);
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
	async function refresh() {
		if (!get(can).has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL)) return;
		const { revision, signal } = requests.begin();
		loading = true;
		error = '';
		try {
			const result = await getPromotionRun(runId, { signal });
			if (!alive || !requests.isCurrent(revision)) return;
			workspace = result;
			if (!options) {
				const references = await getPromotionPolicyOptions({ signal });
				if (alive && requests.isCurrent(revision)) options = references;
			}
		} catch (cause) {
			if (alive && requests.isCurrent(revision) && !isAbortError(cause)) error = message(cause);
		} finally {
			if (alive && requests.isCurrent(revision)) loading = false;
		}
	}
	async function loadRooms() {
		if (!workspace || !editable) return;
		const revision = ++roomRevision;
		roomsLoading = true;
		try {
			const result = await lookupHomerooms({
				academicYearId: workspace.targetYear.academicYearId,
				activeOnly: true,
				limit: 500,
				search: roomSearch
			});
			if (alive && revision === roomRevision) rooms = result;
		} catch (cause) {
			if (alive && revision === roomRevision) formError = message(cause);
		} finally {
			if (alive && revision === roomRevision) roomsLoading = false;
		}
	}
	async function reloadForReview() {
		if (busy || saving || selected) return;
		await refresh();
		if (!error && alive) {
			// Explicit reload starts a new review of canonical versions. Network retry
			// without reloading still retains the original idempotency key.
			calculateInput = null;
			approveInput = null;
			executeInput = null;
		}
	}
	function openStudent(row: PromotionRunStudent) {
		selected = row;
		draft = initialDecision(row.item);
		formError = '';
		roomSearch = '';
		rooms = [];
		// Load action-only references after the modal's capability is evaluated.
	}
	function changeOutcome(value: string) {
		if (!Object.hasOwn(outcomeLabels, value)) return;
		const outcome = Object.keys(outcomeLabels).find((key) => key === value);
		if (!outcome) return;
		for (const key of Object.keys(outcomeLabels) as Array<keyof typeof outcomeLabels>) {
			if (key !== outcome) continue;
			draft.outcome = key;
		}
		if (!decisionUsesDestination(draft.outcome)) {
			draft.targetGradeLevelId = null;
			draft.targetStudyProgramId = null;
			draft.targetHomeroomId = null;
		} else if (draft.outcome === 'repeat' && selected)
			draft.targetGradeLevelId = selected.item.sourceGradeLevelId;
		if (draft.outcome !== 'conditional') draft.condition = null;
	}
	async function saveReview() {
		if (!selected || !workspace || !editable || saving || validation || !draft.outcome) return;
		saving = true;
		formError = '';
		try {
			const result = await reviewPromotionItem(runId, selected.item.id, {
				rowVersion: selected.item.rowVersion,
				decision: { ...draft, outcome: draft.outcome }
			});
			if (!alive) return;
			workspace = {
				...workspace,
				run: result.run,
				students: workspace.students.map((row) =>
					row.item.id === result.item.id ? { ...row, item: result.item } : row
				)
			};
			selected = null;
			// Approval is a checksum of the whole reviewed set, not just this row.
			if (result.run.status === 'reviewed') await refresh();
		} catch (cause) {
			if (alive) formError = message(cause);
		} finally {
			if (alive) saving = false;
		}
	}
	async function perform() {
		const action = confirmation;
		if (!workspace || !action || busy || saving) return;
		if (
			(action === 'calculate' && !manage) ||
			(action === 'approve' && (!approve || !canApproveRun(workspace))) ||
			(action === 'execute' && (!execute || !canExecuteRun(workspace)))
		)
			return;
		busy = action;
		confirmation = null;
		error = '';
		stopRequested = false;
		try {
			if (action === 'calculate') {
				calculateInput ??= { requestId: crypto.randomUUID(), rowVersion: workspace.run.rowVersion };
				const result = await calculatePromotionRun(runId, calculateInput);
				calculateInput = null;
				if (alive) {
					workspace = { ...workspace, run: result.run };
					await refresh();
				}
			} else if (action === 'approve') {
				approveInput ??= {
					requestId: crypto.randomUUID(),
					rowVersion: workspace.run.rowVersion,
					sourceChecksum: workspace.approvalChecksum
				};
				const result = await approvePromotionRun(runId, approveInput);
				approveInput = null;
				if (alive) workspace = { ...workspace, run: result };
			} else {
				do {
					executeInput ??= {
						requestId: crypto.randomUUID(),
						rowVersion: workspace.run.rowVersion,
						limit: 100
					};
					const result = await executePromotionRun(runId, executeInput);
					executeInput = null;
					if (!alive) return;
					workspace = {
						...workspace,
						run: result.run,
						students: workspace.students.map((row) => {
							const receipt = result.receipts.find((entry) => entry.itemId === row.item.id);
							return receipt ? { ...row, receipt } : row;
						})
					};
					progress = `เหลือ ${result.remainingCount} รายการ · พักรายการ ${result.holdCount} รายการ`;
					if (result.failures.length) {
						error = result.failures
							.map(
								(failure) =>
									`${workspace?.students.find((row) => row.item.id === failure.itemId)?.studentCode ?? 'นักเรียน'}: ${failure.message}`
							)
							.join('\n');
						break;
					}
					if (!result.remainingCount) break;
				} while (
					alive &&
					!stopRequested &&
					get(can).has(PERMISSIONS.ACADEMIC_PROMOTION_EXECUTE_SCHOOL)
				);
				// Whole-run mutations may have advanced several item versions.
				const executionError = error;
				if (alive) await refresh();
				if (alive && executionError) error = executionError;
			}
		} catch (cause) {
			if (alive) error = message(cause);
		} finally {
			if (alive) busy = null;
		}
	}
	onMount(() => {
		const unsubscribe = can.subscribe((value) => {
			if (!loaded && value.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL)) {
				loaded = true;
				void refresh();
			}
		});
		const unregister = registerAcademicContextDirtySource(
			`promotion-${runId}`,
			() => !!selected || !!busy || saving
		);
		return () => {
			alive = false;
			roomRevision++;
			requests.abort();
			unsubscribe();
			unregister();
		};
	});
</script>

<PageShell
	title="ตรวจรอบเลื่อนชั้น"
	description="ตรวจผลทีละคน อนุมัติทั้งรอบ แล้วจึงสร้างข้อมูลปีใหม่ โดยเก็บข้อมูลปีเดิมไว้"
>
	{#snippet actions()}
		<Button variant="outline" href={resolve('/staff/academic/promotion')}>กลับรายการรอบ</Button>
		<Button
			variant="outline"
			aria-label="โหลดข้อมูลรอบใหม่"
			disabled={loading || !!busy || saving || !!selected}
			onclick={reloadForReview}><RefreshCw class="size-4" /></Button
		>
	{/snippet}
	{#if !read}<PageState variant="permission" title="ไม่มีสิทธิ์ดูรอบเลื่อนชั้น" />
	{:else}
		{#if error}<div
				role="alert"
				class="whitespace-pre-line rounded-xl border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive"
			>
				{error}
			</div>{/if}
		{#if loading && !workspace}<PageSkeleton variant="table" rows={5} columns={5} />
		{:else if workspace}
			<div
				class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card p-3 sm:p-4"
			>
				<div class="flex items-center gap-4">
					<div>
						<p class="text-xs text-muted-foreground">ปีต้นทาง</p>
						<p class="font-semibold">{workspace.sourceYear.name}</p>
					</div>
					<ArrowRight class="size-4 text-muted-foreground" />
					<div>
						<p class="text-xs text-muted-foreground">เตรียมข้อมูลให้</p>
						<p class="font-semibold text-primary">{workspace.targetYear.name}</p>
					</div>
				</div>
				<Badge variant="outline"
					>{runStatusLabels[workspace.run.status]}{#if completedHolds}
						· พักรายการ {completedHolds} คน{/if}</Badge
				>
				<div class="flex flex-wrap gap-2">
					{#if workspace.students.some((row) => row.receipt)}
						{#key runId}<PromotionImpactsDialog
								{runId}
								students={workspace.students}
								{options}
								disabled={!!busy || loading}
							/>{/key}
					{/if}
					{#if manage && !['executing', 'completed'].includes(workspace.run.status)}<Button
							variant="outline"
							disabled={!!busy || loading}
							onclick={() => (confirmation = 'calculate')}>คำนวณข้อเสนอ</Button
						>{/if}
					{#if approve}<Button
							disabled={!!busy || loading || !canApproveRun(workspace)}
							onclick={() => (confirmation = 'approve')}>อนุมัติรอบ</Button
						>{/if}
					{#if execute}<Button
							disabled={!!busy || loading || !canExecuteRun(workspace)}
							onclick={() => (confirmation = 'execute')}>ดำเนินการเลื่อนชั้น</Button
						>{/if}
				</div>
			</div>
			<p class="text-sm text-muted-foreground">
				ข้อมูลปีใหม่จะอยู่ในสถานะเตรียมการ
				การดำเนินการรอบนี้ไม่ได้เปิดปีหรือภาคเรียนใหม่โดยอัตโนมัติ
			</p>
			{#if busy === 'execute' || progress}<div
					aria-live="polite"
					class="flex flex-wrap items-center gap-3 text-sm"
				>
					<span>{progress || 'กำลังดำเนินการชุดแรก…'}</span>{#if busy === 'execute'}<Button
							variant="outline"
							disabled={stopRequested}
							onclick={() => (stopRequested = true)}>หยุดหลังชุดนี้</Button
						>{/if}
				</div>{/if}
			<div class="flex flex-wrap items-center justify-between gap-3">
				<Input
					aria-label="ค้นหานักเรียนในรอบ"
					placeholder="ค้นหารหัสหรือชื่อนักเรียน"
					class="max-w-sm"
					value={search}
					oninput={(event) => {
						search = event.currentTarget.value;
						visibleCount = 50;
					}}
				/><span class="text-sm text-muted-foreground">{filtered.length} คน</span>
			</div>
			<div class="overflow-x-auto rounded-xl border bg-card">
				<Table.Root class="min-w-[850px]">
					<Table.Header
						><Table.Row
							><Table.Head>นักเรียน</Table.Head><Table.Head>ชั้นและแผนเดิม</Table.Head><Table.Head
								class="bg-primary/5">ปลายทางที่พิจารณา</Table.Head
							><Table.Head>สถานะ</Table.Head><Table.Head
								><span class="sr-only">พิจารณา</span></Table.Head
							></Table.Row
						></Table.Header
					>
					<Table.Body
						>{#each visible as row (row.item.id)}{@const decision =
								row.item.decision ?? row.item.recommendation}
							<Table.Row
								><Table.Cell
									><p class="font-medium">{row.studentName}</p>
									<p class="text-xs text-muted-foreground">
										{row.studentCode ?? 'ยังไม่มีรหัสนักเรียน'}
									</p></Table.Cell
								><Table.Cell
									><p>{gradeName(row.item.sourceGradeLevelId)}</p>
									<p class="text-xs text-muted-foreground">
										{programName(row.item.sourceStudyProgramId)}
									</p></Table.Cell
								><Table.Cell class="bg-primary/5"
									>{#if row.item.decision && !decisionUsesDestination(row.item.decision.outcome)}<p>
											ไม่สร้างข้อมูลปีใหม่
										</p>{:else}<p>{gradeName(decision.targetGradeLevelId)}</p>
										<p class="text-xs text-muted-foreground">
											{programName(decision.targetStudyProgramId)}
										</p>{/if}
									<p class="mt-1 text-xs font-medium">
										{row.item.decision
											? outcomeLabels[row.item.decision.outcome]
											: 'ข้อเสนอ · ยังไม่ยืนยันผลพิจารณา'}
									</p></Table.Cell
								><Table.Cell
									>{#if row.receipt || row.item.status === 'executed'}ดำเนินการแล้ว{:else if row.needsRecalculation}ต้องคำนวณใหม่{:else if !row.annualResultCurrent}ผลรายปียังไม่พร้อม{:else if row.item.status === 'reviewed'}ตรวจแล้ว{:else if row.item.status === 'failed'}ต้องลองใหม่{:else}รอตรวจ{/if}</Table.Cell
								><Table.Cell
									><Button
										variant="outline"
										size="sm"
										disabled={!!busy || saving}
										onclick={() => openStudent(row)}
										>{manage && row.item.status !== 'executed' ? 'พิจารณา' : 'รายละเอียด'}</Button
									></Table.Cell
								></Table.Row
							>
						{/each}</Table.Body
					>
				</Table.Root>
			</div>
			{#if !filtered.length}<PageState
					variant="empty"
					title={workspace.run.status === 'draft' ? 'เริ่มจากคำนวณข้อเสนอ' : 'ไม่พบรายการนักเรียน'}
				/>{/if}
			{#if visibleCount < filtered.length}<Button
					variant="outline"
					onclick={() => (visibleCount += 50)}>แสดงนักเรียนเพิ่ม</Button
				>{/if}
		{/if}
	{/if}
</PageShell>

<Dialog.Root
	open={!!selected}
	onOpenChange={(open) => {
		if (!open && !saving) {
			selected = null;
			roomRevision++;
		}
	}}
>
	<Dialog.Content class="max-h-[85dvh] overflow-y-auto sm:max-w-xl">
		<Dialog.Header
			><Dialog.Title>ผลพิจารณารายบุคคล</Dialog.Title><Dialog.Description
				>{selected?.studentName} · {selected?.studentCode ?? ''}</Dialog.Description
			></Dialog.Header
		>
		{#if selected}
			{#if !selected.item.decision}<p class="text-sm text-muted-foreground">
					ข้อมูลด้านล่างเป็นข้อเสนอ ยังไม่ได้บันทึกผลพิจารณา
				</p>{/if}
			<p class="text-sm">
				ต้นทาง: {gradeName(selected.item.sourceGradeLevelId)} · {programName(
					selected.item.sourceStudyProgramId
				)}
			</p>
			{#each selected.item.recommendation.findings as finding (finding)}<p
					class="text-sm text-amber-700 dark:text-amber-400"
				>
					{findingLabels[finding]}
				</p>{/each}
			{#if !editable}<p class="text-sm text-muted-foreground">
					ดูรายละเอียดเท่านั้น หากข้อมูลต้นทางเปลี่ยน ต้องคำนวณใหม่ก่อนพิจารณา
				</p>{/if}
			<div class="space-y-2">
				<Label for="promotion-outcome">ผลพิจารณา</Label><Select.Root
					type="single"
					value={draft.outcome}
					onValueChange={changeOutcome}
					disabled={!editable || saving}
					><Select.Trigger id="promotion-outcome" class="w-full"
						>{draft.outcome ? outcomeLabels[draft.outcome] : 'เลือกผลพิจารณา'}</Select.Trigger
					><Select.Content
						>{#each Object.entries(outcomeLabels) as [value, label] (value)}<Select.Item {value}
								>{label}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
			</div>
			{#if decisionUsesDestination(draft.outcome)}
				<div class="space-y-2">
					<Label for="promotion-grade">ชั้นปลายทาง</Label><Select.Root
						type="single"
						value={draft.targetGradeLevelId ?? ''}
						onValueChange={(value) => {
							draft.targetGradeLevelId = value || null;
							draft.targetHomeroomId = null;
						}}
						disabled={!editable || saving || draft.outcome === 'repeat'}
						><Select.Trigger id="promotion-grade" class="w-full"
							>{gradeName(draft.targetGradeLevelId)}</Select.Trigger
						><Select.Content
							>{#each options?.grades.filter((grade) => grade.isActive) ?? [] as grade (grade.id)}<Select.Item
									value={grade.id}>{gradeName(grade.id)}</Select.Item
								>{/each}</Select.Content
						></Select.Root
					>
				</div>
				<div class="space-y-2">
					<Label for="promotion-program">แผนการเรียนปลายทาง</Label><Select.Root
						type="single"
						value={draft.targetStudyProgramId ?? ''}
						onValueChange={(value) => {
							draft.targetStudyProgramId = value || null;
							draft.targetHomeroomId = null;
						}}
						disabled={!editable || saving}
						><Select.Trigger id="promotion-program" class="w-full"
							>{programName(draft.targetStudyProgramId)}</Select.Trigger
						><Select.Content
							>{#each options?.programs.filter((program) => program.status === 'published') ?? [] as program (program.id)}<Select.Item
									value={program.id}
									>{program.name} · {program.curriculumName} · {program.versionName}</Select.Item
								>{/each}</Select.Content
						></Select.Root
					>
				</div>
				<div class="space-y-2">
					<Label for="promotion-room">ห้องปลายทาง (จัดภายหลังได้)</Label>
					<div class="flex gap-2">
						<Input
							aria-label="ค้นหาห้องปลายทาง"
							bind:value={roomSearch}
							disabled={!editable || saving}
							placeholder="ค้นหาชื่อห้อง"
						/><LoadingButton
							variant="outline"
							loading={roomsLoading}
							disabled={!editable || saving}
							onclick={loadRooms}>ค้นหาห้อง</LoadingButton
						>
					</div>
					<Select.Root
						type="single"
						value={draft.targetHomeroomId ?? 'later'}
						onValueChange={(value) => (draft.targetHomeroomId = value === 'later' ? null : value)}
						disabled={!editable || saving}
						><Select.Trigger id="promotion-room" class="w-full"
							>{draft.targetHomeroomId
								? (rooms.find((room) => room.id === draft.targetHomeroomId)?.name ??
									'ห้องที่บันทึกไว้')
								: 'จัดห้องภายหลัง'}</Select.Trigger
						><Select.Content
							><Select.Item value="later">จัดห้องภายหลัง</Select.Item
							>{#each eligibleRooms as room (room.id)}<Select.Item value={room.id}
									>{room.name}</Select.Item
								>{/each}</Select.Content
						></Select.Root
					>
					<p class="text-xs text-muted-foreground">
						ห้องต้องเป็นชั้นและแผนเดียวกับปลายทาง ระบบตรวจซ้ำก่อนบันทึกและดำเนินการ
					</p>
				</div>
			{/if}
			<div class="space-y-2">
				<Label for="promotion-reason">เหตุผล</Label><Textarea
					id="promotion-reason"
					value={draft.reason ?? ''}
					oninput={(event) => (draft.reason = event.currentTarget.value || null)}
					disabled={!editable || saving}
					maxlength={1000}
				/>
			</div>
			{#if draft.outcome === 'conditional'}<div class="space-y-2">
					<Label for="promotion-condition">เงื่อนไขที่ต้องติดตาม</Label><Textarea
						id="promotion-condition"
						value={draft.condition ?? ''}
						oninput={(event) => (draft.condition = event.currentTarget.value || null)}
						disabled={!editable || saving}
						maxlength={1000}
					/>
				</div>{/if}
			{#if validation && editable}<p class="text-sm text-amber-700 dark:text-amber-400">
					{validation}
				</p>{/if}
			{#if formError}<p role="alert" class="text-sm text-destructive">{formError}</p>{/if}
			<Dialog.Footer
				><Button variant="outline" disabled={saving} onclick={() => (selected = null)}>ปิด</Button
				>{#if editable}<LoadingButton loading={saving} disabled={!!validation} onclick={saveReview}
						>บันทึกผลพิจารณา</LoadingButton
					>{/if}</Dialog.Footer
			>
		{/if}
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root
	open={!!confirmation}
	onOpenChange={(open) => {
		if (!open) confirmation = null;
	}}
	><Dialog.Content
		><Dialog.Header
			><Dialog.Title
				>{confirmation === 'calculate'
					? 'คำนวณข้อเสนอใหม่'
					: confirmation === 'approve'
						? 'อนุมัติผลทั้งรอบ'
						: 'ดำเนินการตามผลที่อนุมัติ'}</Dialog.Title
			><Dialog.Description
				>{workspace?.sourceYear.name} → {workspace?.targetYear.name}</Dialog.Description
			></Dialog.Header
		>
		<p class="text-sm">
			{confirmation === 'calculate'
				? 'คำนวณจากผลรายปีล่าสุด และล้างผลพิจารณาของรายการที่ยังไม่ดำเนินการ เพื่อให้ตรวจใหม่ ข้อมูลที่ดำเนินการแล้วจะคงเดิม'
				: confirmation === 'approve'
					? 'ยืนยันผลพิจารณาของนักเรียนที่ตรวจครบในรอบนี้ การอนุมัติยังไม่สร้างข้อมูลปีใหม่'
					: 'จะสร้างข้อมูลนักเรียนปีใหม่ หรือเปลี่ยนสถานะจบ/ย้ายออกตามผลที่อนุมัติ โดยทำทีละชุด หากขัดแย้งจะหยุดให้ตรวจสอบ ส่วนที่สำเร็จแล้วไม่ทำซ้ำ'}
		</p>
		<Dialog.Footer
			><Button variant="outline" onclick={() => (confirmation = null)}>ยกเลิก</Button><Button
				onclick={perform}>ยืนยัน</Button
			></Dialog.Footer
		></Dialog.Content
	></Dialog.Root
>
