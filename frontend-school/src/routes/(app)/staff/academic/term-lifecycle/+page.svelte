<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { ArrowUpRight, RefreshCw, ShieldCheck } from 'lucide-svelte';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import {
		canConfirmTransition,
		termStatusLabels,
		transitionLabels
	} from '$lib/academic/lifecycle/presentation';
	import {
		getTermLifecycleWorkspace,
		transitionAcademicTerm,
		type TermLifecycleWorkspace,
		type TermTransitionAction,
		type TermTransitionRequest
	} from '$lib/api/academic-lifecycle';
	import { ApiClientError } from '$lib/api/client';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import OpeningPolicyDialog from '$lib/components/academic/lifecycle/OpeningPolicyDialog.svelte';
	import TermActivationDialog from '$lib/components/academic/lifecycle/TermActivationDialog.svelte';
	import TermPreparationDialog from '$lib/components/academic/lifecycle/TermPreparationDialog.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const academicContext = getAcademicContextStore();
	const latest = new LatestRequest();
	const permissions: Record<TermTransitionAction, string> = {
		mark_ready: PERMISSIONS.ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
		begin_closing: PERMISSIONS.ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
		cancel_closing: PERMISSIONS.ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
		cancel: PERMISSIONS.ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
		close: PERMISSIONS.ACADEMIC_LIFECYCLE_CLOSE_SCHOOL,
		reopen: PERMISSIONS.ACADEMIC_LIFECYCLE_REOPEN_SCHOOL,
		activate: PERMISSIONS.ACADEMIC_LIFECYCLE_ACTIVATE_SCHOOL
	};
	const explanations: Record<TermTransitionAction, string> = {
		mark_ready: 'ระบุว่าภาคเรียนเตรียมพร้อมแล้ว ยังไม่เปลี่ยนภาคเรียนที่กำลังใช้งาน',
		begin_closing: 'เริ่มตรวจงานก่อนปิดภาคเรียน ครูยังทำงานที่เปิดให้แก้ไขอยู่ได้ตามเดิม',
		cancel_closing: 'กลับไปใช้งานภาคเรียนตามปกติ ไม่เปลี่ยนการล็อกผลหรือช่วงเปิดกรอกคะแนน',
		close:
			'หยุดการแก้ไขข้อมูลภาคเรียนผ่านงานปกติ ผลและประวัติเดิมยังอยู่ การแก้ผลใช้หน้าของฝ่ายวิชาการ',
		reopen: 'เปิดกลับสู่สถานะกำลังปิด ไม่ปลดล็อกผลรายวิชาและไม่เปิดช่องกรอกคะแนนให้อัตโนมัติ',
		cancel: 'ยกเลิกเฉพาะภาคเรียนที่ยังวางแผนอยู่ โดยเก็บข้อมูลเดิมไว้',
		activate:
			'เปลี่ยนภาคเรียนที่ระบบใช้เป็นค่าเริ่มต้น ต้องปิดภาคเรียนก่อนหน้าแล้ว ไม่ใช่แค่เลือกดูปีบน Topbar'
	};
	const steps = ['planning', 'ready', 'active', 'closing', 'closed'] as const;
	let workspace = $state.raw<TermLifecycleWorkspace | null>(null);
	let loading = $state(false);
	let loadError = $state('');
	let busy = $state(false);
	let dialogOpen = $state(false);
	let action = $state<TermTransitionAction>('begin_closing');
	let closedOn = $state<string | undefined>();
	let reason = $state('');
	let acknowledged = $state<string[]>([]);
	let mutationError = $state('');
	let needsRefresh = $state(false);
	let pending: { key: string; request: TermTransitionRequest } | null = null;
	let loadedContextKey = '';
	const canRead = $derived($can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL));
	const yearId = $derived($academicContext.selected.academicYearId);
	const termId = $derived($academicContext.selected.academicTermId);
	const availableActions = $derived(
		workspace?.availableActions.filter(
			(candidate) => candidate !== 'activate' && $can.has(permissions[candidate])
		) ?? []
	);
	const blocked = $derived(
		workspace?.findings.some((finding) => finding.severity === 'blocking') ?? true
	);
	const warnings = $derived(
		workspace?.findings.filter((finding) => finding.severity === 'warning') ?? []
	);
	const confirmable = $derived(
		!busy &&
			!loading &&
			!needsRefresh &&
			!!workspace &&
			$can.has(permissions[action]) &&
			canConfirmTransition(workspace, action, closedOn, reason, acknowledged)
	);

	async function loadWorkspace() {
		if (!yearId || !termId || !canRead) return;
		const { revision, signal } = latest.begin();
		loading = true;
		loadError = '';
		try {
			const next = await getTermLifecycleWorkspace(termId, { academicYearId: yearId }, { signal });
			if (!latest.isCurrent(revision)) return;
			workspace = next;
			needsRefresh = false;
			acknowledged = [];
		} catch (error) {
			if (isAbortError(error)) return;
			if (latest.isCurrent(revision))
				loadError = error instanceof Error ? error.message : 'โหลดความพร้อมไม่สำเร็จ';
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}
	function openAction(next: TermTransitionAction) {
		if (busy || !availableActions.includes(next)) return;
		action = next;
		closedOn = undefined;
		reason = '';
		acknowledged = [];
		mutationError = '';
		needsRefresh = false;
		pending = null;
		dialogOpen = true;
	}
	async function confirm() {
		if (!confirmable || !workspace || !termId) return;
		const selectedTerm = termId;
		const selectedYear = workspace.context.academicYearId;
		const body = {
			academicYearId: selectedYear,
			action,
			expectedYearVersion: workspace.context.yearRowVersion,
			expectedTermVersion: workspace.context.termRowVersion,
			readinessChecksum: workspace.sourceChecksum,
			acknowledgedWarningCodes: [...acknowledged].sort(),
			closedOn: closedOn ?? null,
			reason: action === 'reopen' ? reason.trim() : null
		};
		const key = JSON.stringify(body);
		if (pending?.key !== key)
			pending = { key, request: { ...body, requestId: crypto.randomUUID() } };
		busy = true;
		mutationError = '';
		try {
			const outcome = await transitionAcademicTerm(selectedTerm, pending.request);
			if (termId === selectedTerm && yearId === selectedYear) {
				workspace = { ...workspace, context: outcome.context, availableActions: [] };
				dialogOpen = false;
				toast.success(`${transitionLabels[outcome.action]}แล้ว`);
				await academicContext.retry();
				await loadWorkspace();
			}
			pending = null;
		} catch (error) {
			mutationError = error instanceof Error ? error.message : 'เปลี่ยนสถานะไม่สำเร็จ';
			if (error instanceof ApiClientError && error.status === 409) needsRefresh = true;
		} finally {
			busy = false;
		}
	}
	async function refreshAfterActivation() {
		await academicContext.retry();
		await loadWorkspace();
	}
	onMount(() => {
		const unregister = registerAcademicContextDirtySource(
			'term-lifecycle',
			() => dialogOpen || busy
		);
		const unsubscribe = academicContext.subscribe((state) => {
			const key =
				state.status === 'ready' && state.selected.academicYearId && state.selected.academicTermId
					? `${state.selected.academicYearId}:${state.selected.academicTermId}`
					: '';
			if (key && key !== loadedContextKey) {
				loadedContextKey = key;
				dialogOpen = false;
				workspace = null;
				void loadWorkspace();
			} else if (!key && state.status !== 'loading') {
				loadedContextKey = '';
				latest.abort();
				workspace = null;
				dialogOpen = false;
			}
		});
		return () => {
			unsubscribe();
			unregister();
			latest.abort();
		};
	});
</script>

<PageShell
	title="ปิดและเปลี่ยนภาคเรียน"
	description="ตรวจงานก่อนปิดภาคเรียน แล้วเริ่มภาคเรียนถัดไปเมื่อพร้อม"
>
	{#snippet actions()}
		<OpeningPolicyDialog
			disabled={busy || loading}
			onupdated={async () => {
				await loadWorkspace();
			}}
		/>
		<Button
			variant="outline"
			disabled={busy || loading || !yearId || !termId}
			onclick={() => void loadWorkspace()}><RefreshCw class="size-4" />ตรวจข้อมูลล่าสุด</Button
		>
	{/snippet}
	{#if !canRead}
		<PageState variant="permission" title="ไม่มีสิทธิ์ดูความพร้อมระดับโรงเรียน" />
	{:else if !yearId || !termId}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาและภาคเรียนก่อน"
			description="เลือกบริบทบนแถบด้านบน การเลือกดูข้อมูลไม่เปลี่ยนภาคเรียนที่กำลังใช้งาน"
		/>
	{:else if loading && !workspace}
		<PageSkeleton variant="table" rows={5} columns={3} />
	{:else if loadError && !workspace}
		<PageState
			variant="error"
			title="โหลดความพร้อมไม่สำเร็จ"
			description={loadError}
			actionLabel="ลองอีกครั้ง"
			onaction={() => void loadWorkspace()}
		/>
	{:else if workspace}
		<section class="rounded-xl border bg-card p-4 sm:p-5" aria-label="สถานะภาคเรียน">
			<div class="mb-4 flex flex-wrap items-center justify-between gap-3">
				<div>
					<p class="text-sm text-muted-foreground">{workspace.context.yearName}</p>
					<h2 class="text-lg font-semibold">{workspace.context.termName}</h2>
				</div>
				<span class="rounded-full bg-primary/10 px-3 py-1 text-sm font-medium text-primary"
					>{termStatusLabels[workspace.context.termStatus]}</span
				>
			</div>
			<ol class="flex flex-wrap gap-2 text-sm" aria-label="ลำดับสถานะ">
				{#each steps as step (step)}<li
						aria-current={workspace.context.termStatus === step ? 'step' : undefined}
						class={[
							'rounded-md border px-3 py-2',
							workspace.context.termStatus === step
								? 'border-primary bg-primary text-primary-foreground'
								: 'text-muted-foreground'
						]}
					>
						{termStatusLabels[step]}
					</li>{/each}
			</ol>
		</section>
		{#if loadError}<p role="alert" class="text-sm text-destructive">{loadError}</p>{/if}
		<div class="grid items-start gap-5 lg:grid-cols-[minmax(0,1fr)_20rem]">
			<section class="overflow-hidden rounded-xl border bg-card" aria-labelledby="readiness-title">
				<div class="border-b p-4">
					<h2 id="readiness-title" class="font-semibold">ความพร้อมก่อนปิดภาคเรียน</h2>
					<p class="mt-1 text-sm text-muted-foreground">
						ผลสรุปล่าสุดครบ <span class="font-mono tabular-nums"
							>{workspace.coverage.students.filter((student) => student.isCurrent).length} / {workspace
								.coverage.students.length}</span
						> คน
					</p>
				</div>
				{#if workspace.findings.length === 0 && workspace.coverage.ready}
					<p class="flex items-center gap-2 p-4 text-sm">
						<ShieldCheck class="size-5 text-emerald-700" />ผลสรุปครบแล้ว ไม่พบงานค้างที่ต้องยืนยัน
					</p>
				{:else}
					<ul class="divide-y">
						{#each workspace.findings as finding (finding.code)}
							<li class="flex flex-wrap items-start gap-3 p-4">
								<span
									class={[
										'rounded-md px-2 py-1 text-xs font-medium',
										finding.severity === 'blocking'
											? 'bg-destructive/10 text-destructive'
											: 'bg-amber-50 text-amber-900'
									]}>{finding.severity === 'blocking' ? 'ต้องแก้ก่อนปิด' : 'ตรวจและยืนยัน'}</span
								>
								<div class="min-w-0 flex-1">
									<p class="text-sm">{finding.message}</p>
									<p class="mt-1 text-xs text-muted-foreground">{finding.count} รายการ</p>
								</div>
								{#if finding.resolutionUrl}<Button
										variant="ghost"
										size="sm"
										href={finding.resolutionUrl}>ไปตรวจสอบ<ArrowUpRight class="size-4" /></Button
									>{/if}
							</li>
						{/each}
					</ul>
				{/if}
			</section>
			<aside class="space-y-3" aria-label="การดำเนินการภาคเรียน">
				<h2 class="font-semibold">ดำเนินการกับภาคเรียนนี้</h2>
				<p class="text-sm text-muted-foreground">
					การปิดภาคเรียนแยกจากการเปิด–ปิดช่องกรอกคะแนนและการล็อกผลรายวิชา
				</p>
				{#if workspace.context.termStatus === 'closed'}
					<TermPreparationDialog
						sourceTermId={workspace.context.academicTermId}
						disabled={busy || loading}
						onapplied={loadWorkspace}
					/>
				{/if}
				{#if workspace.context.termStatus === 'ready'}
					<TermActivationDialog
						yearId={workspace.context.academicYearId}
						termId={workspace.context.academicTermId}
						disabled={busy || loading}
						onactivated={refreshAfterActivation}
					/>
				{/if}
				{#each availableActions as candidate (candidate)}
					<Button
						class="w-full justify-start whitespace-normal text-start"
						variant={candidate === 'close' ? 'destructive' : 'outline'}
						disabled={busy ||
							loading ||
							(candidate === 'close' && (blocked || !workspace.coverage.ready))}
						onclick={() => openAction(candidate)}>{transitionLabels[candidate]}</Button
					>
				{/each}
				{#if availableActions.length === 0 && workspace.context.termStatus !== 'ready'}<p
						class="rounded-lg border border-dashed p-3 text-sm text-muted-foreground"
					>
						ดูสถานะได้ ยังไม่มีคำสั่งที่ใช้ได้ตามสถานะและสิทธิ์ของบัญชีนี้
					</p>{/if}
			</aside>
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={dialogOpen}>
	<Dialog.Content class="max-h-[85dvh] overflow-y-auto">
		<Dialog.Header
			><Dialog.Title>{transitionLabels[action]}</Dialog.Title><Dialog.Description
				>{explanations[action]}</Dialog.Description
			></Dialog.Header
		>
		{#if workspace}
			<p class="text-sm font-medium">{workspace.context.yearName} · {workspace.context.termName}</p>
			{#if action === 'close'}
				<div class="space-y-2">
					<Label for="actual-close-date">วันที่ปิดภาคเรียนจริง</Label><DatePicker
						id="actual-close-date"
						ariaLabel="วันที่ปิดภาคเรียนจริง"
						bind:value={closedOn}
						disabled={busy}
					/>
					<p class="text-xs text-muted-foreground">
						ต้องไม่ก่อนวันเริ่มภาคเรียนและไม่เกินวันสิ้นสุดปีการศึกษา
					</p>
				</div>
				{#each warnings as warning (warning.code)}<div
						class="flex items-start gap-3 rounded-lg border p-3"
					>
						<Checkbox
							id={`ack-${warning.code}`}
							checked={acknowledged.includes(warning.code)}
							disabled={busy}
							onCheckedChange={(checked) => {
								acknowledged = checked
									? [...acknowledged, warning.code]
									: acknowledged.filter((code) => code !== warning.code);
							}}
						/><Label for={`ack-${warning.code}`} class="leading-relaxed"
							>รับทราบ: {warning.message} ({warning.count} รายการ)</Label
						>
					</div>{/each}
			{:else if action === 'reopen'}
				<div class="space-y-2">
					<Label for="reopen-reason">เหตุผลเปิดกลับ</Label><Textarea
						id="reopen-reason"
						bind:value={reason}
						maxlength={1000}
						disabled={busy}
						placeholder="ระบุเรื่องที่ต้องกลับมาตรวจสอบ"
					/>
				</div>
			{/if}
		{/if}
		{#if mutationError}<p role="alert" class="text-sm text-destructive">{mutationError}</p>{/if}
		{#if needsRefresh}<LoadingButton
				variant="outline"
				{loading}
				onclick={() => void loadWorkspace()}>โหลดความพร้อมล่าสุด</LoadingButton
			>{/if}
		<Dialog.Footer
			><Button
				variant="outline"
				disabled={busy}
				onclick={() => {
					dialogOpen = false;
				}}>กลับไปตรวจสอบ</Button
			><LoadingButton loading={busy} disabled={!confirmable} onclick={() => void confirm()}
				>ยืนยัน{transitionLabels[action]}</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
