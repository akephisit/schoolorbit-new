<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { toast } from 'svelte-sonner';
	import { RefreshCw, ArrowUpRight } from 'lucide-svelte';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import { termStatusLabels } from '$lib/academic/lifecycle/presentation';
	import {
		getYearLifecycleWorkspace,
		transitionAcademicYear,
		type YearLifecycleWorkspace,
		type YearTransitionAction,
		type YearTransitionRequest
	} from '$lib/api/academic-lifecycle';
	import { ApiClientError } from '$lib/api/client';
	import type { YearReopeningOutcome } from '$lib/api/academic-lifecycle';
	import YearReopeningDialog from '$lib/components/academic/lifecycle/YearReopeningDialog.svelte';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { PageShell } from '$lib/components/app-layout';
	import { PageState, PageSkeleton, LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Label } from '$lib/components/ui/label';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Table from '$lib/components/ui/table';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const academicContext = getAcademicContextStore();
	const latest = new LatestRequest();
	const labels: Record<YearTransitionAction, string> = {
		begin_closing: 'เริ่มขั้นตอนปิดปี',
		cancel_closing: 'กลับไปใช้งานปีนี้',
		close: 'ปิดปีการศึกษา'
	};
	const explanations: Record<YearTransitionAction, string> = {
		begin_closing: 'เริ่มตรวจความพร้อมปิดปี ไม่เปลี่ยนผลการเรียนหรือเปิดช่องกรอกคะแนน',
		cancel_closing: 'กลับไปสถานะใช้งาน โดยคงการปิดภาคเรียนและผลที่ล็อกไว้ตามเดิม',
		close:
			'ปิดปีนี้เมื่อผลรายปีและภาคเรียนครบแล้ว เก็บข้อมูลเดิมไว้ ไม่เลื่อนชั้นและไม่เปิดปีใหม่อัตโนมัติ'
	};
	const statusLabels: Record<YearLifecycleWorkspace['context']['status'], string> = {
		planning: 'เตรียมปีการศึกษา',
		ready: 'พร้อมเริ่ม',
		active: 'ใช้งาน',
		closing: 'กำลังปิด',
		closed: 'ปิดแล้ว',
		archived: 'เก็บถาวร'
	};
	const permissions: Record<YearTransitionAction, string> = {
		begin_closing: PERMISSIONS.ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
		cancel_closing: PERMISSIONS.ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
		close: PERMISSIONS.ACADEMIC_LIFECYCLE_CLOSE_SCHOOL
	};
	let workspace = $state.raw<YearLifecycleWorkspace | null>(null);
	let loading = $state(false);
	let busy = $state(false);
	let loadError = $state('');
	let mutationError = $state('');
	let dialogOpen = $state(false);
	let action = $state<YearTransitionAction>('begin_closing');
	let acknowledged = $state<string[]>([]);
	let needsRefresh = $state(false);
	let pending: { key: string; request: YearTransitionRequest } | null = null;
	let loadedYear = '';
	const yearId = $derived($academicContext.selected.academicYearId);
	const canRead = $derived($can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL));
	const canReadAnnual = $derived(
		$can.hasAll(
			PERMISSIONS.ACADEMIC_RESULT_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL
		)
	);
	const availableActions = $derived(
		workspace?.availableActions.filter((candidate) => $can.has(permissions[candidate])) ?? []
	);
	const warnings = $derived(workspace?.findings.filter((row) => row.severity === 'warning') ?? []);
	const readyCount = $derived(
		workspace?.coverage.students.filter((row) => row.isCurrent).length ?? 0
	);
	const confirmable = $derived(
		!busy &&
			!loading &&
			!needsRefresh &&
			!!workspace &&
			availableActions.includes(action) &&
			(action !== 'close' ||
				(workspace.canClose &&
					warnings.length === acknowledged.length &&
					warnings.every((row) => acknowledged.includes(row.code))))
	);

	async function loadWorkspace() {
		if (!yearId || !canRead) return;
		const selectedYear = yearId;
		const { revision, signal } = latest.begin();
		loading = true;
		loadError = '';
		try {
			const data = await getYearLifecycleWorkspace(selectedYear, { signal });
			if (!latest.isCurrent(revision)) return;
			workspace = data;
			needsRefresh = false;
			acknowledged = [];
			pending = null;
		} catch (error) {
			if (!isAbortError(error) && latest.isCurrent(revision))
				loadError = error instanceof Error ? error.message : 'โหลดความพร้อมไม่สำเร็จ';
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}
	async function handleReopened(outcome: YearReopeningOutcome) {
		if (!workspace || outcome.context.academicYearId !== yearId) return;
		workspace = { ...workspace, context: outcome.context, availableActions: [] };
		try {
			await academicContext.retry();
		} catch {
			toast.warning('เปิดปีเก่ากลับแล้ว แต่รีเฟรชตัวเลือกปีไม่สำเร็จ กรุณาตรวจข้อมูลล่าสุด');
		}
		await loadWorkspace();
	}
	function openAction(next: YearTransitionAction) {
		if (busy || loading || !availableActions.includes(next)) return;
		action = next;
		acknowledged = [];
		mutationError = '';
		needsRefresh = false;
		pending = null;
		dialogOpen = true;
	}
	async function confirm() {
		if (!confirmable || !workspace || !yearId) return;
		const selectedYear = yearId;
		const body = {
			action,
			expectedYearVersion: workspace.context.rowVersion,
			readinessChecksum: workspace.sourceChecksum,
			acknowledgedWarningCodes: [...acknowledged].sort()
		};
		const key = JSON.stringify(body);
		if (pending?.key !== key)
			pending = { key, request: { ...body, requestId: crypto.randomUUID() } };
		busy = true;
		mutationError = '';
		try {
			const outcome = await transitionAcademicYear(selectedYear, pending.request);
			if (yearId === selectedYear) {
				workspace = { ...workspace, context: outcome.context, availableActions: [] };
				dialogOpen = false;
				toast.success(`${labels[outcome.action]}แล้ว`);
				await academicContext.retry();
				await loadWorkspace();
			}
			pending = null;
		} catch (error) {
			mutationError = error instanceof Error ? error.message : 'เปลี่ยนสถานะปีไม่สำเร็จ';
			if (error instanceof ApiClientError && error.status === 409) needsRefresh = true;
		} finally {
			busy = false;
		}
	}
	onMount(() => {
		const unregister = registerAcademicContextDirtySource(
			'year-lifecycle',
			() => dialogOpen || busy
		);
		const unsubscribe = academicContext.subscribe((state) => {
			const key = state.status === 'ready' ? (state.selected.academicYearId ?? '') : '';
			if (key && key !== loadedYear) {
				loadedYear = key;
				workspace = null;
				dialogOpen = false;
				pending = null;
				void loadWorkspace();
			} else if (!key && state.status !== 'loading') {
				loadedYear = '';
				latest.abort();
				workspace = null;
				dialogOpen = false;
				pending = null;
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
	title="ปิดปีการศึกษา"
	description="ตรวจผลรายปีและภาคเรียนก่อนปิดปี โดยไม่เลื่อนชั้นหรือเปิดปีถัดไปอัตโนมัติ"
>
	{#snippet actions()}
		<Button
			variant="outline"
			disabled={busy || loading || !yearId || !canRead}
			onclick={() => void loadWorkspace()}><RefreshCw class="size-4" />ตรวจข้อมูลล่าสุด</Button
		>
	{/snippet}
	{#if !canRead}
		<PageState variant="permission" title="ไม่มีสิทธิ์ดูความพร้อมระดับโรงเรียน" />
	{:else if !yearId}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาก่อน"
			description="เลือกปีที่ต้องการตรวจบนแถบด้านบน การเลือกดูข้อมูลไม่เปลี่ยนปีที่กำลังใช้งาน"
		/>
	{:else if loading && !workspace}
		<PageSkeleton variant="table" rows={4} columns={4} />
	{:else if loadError && !workspace}
		<PageState
			variant="error"
			title="โหลดความพร้อมไม่สำเร็จ"
			description={loadError}
			actionLabel="ลองอีกครั้ง"
			onaction={() => void loadWorkspace()}
		/>
	{:else if workspace}
		<section class="rounded-xl border bg-card p-4" aria-label="สถานะปีการศึกษา">
			<div class="flex flex-wrap items-center justify-between gap-3">
				<div>
					<h2 class="text-lg font-semibold">{workspace.context.name}</h2>
					<p class="mt-1 text-sm text-muted-foreground">
						ปิดภาคเรียนครบแล้ว ปีการศึกษาจะยังไม่ปิดจนกว่าจะยืนยันที่หน้านี้
					</p>
				</div>
				<span class="rounded-md bg-primary/10 px-3 py-1 text-sm font-medium text-primary"
					>{statusLabels[workspace.context.status]}</span
				>
			</div>
		</section>
		{#if loadError}<p role="alert" class="text-sm text-destructive">{loadError}</p>{/if}
		<div class="grid items-start gap-5 xl:grid-cols-[minmax(0,1fr)_20rem]">
			<div class="min-w-0 space-y-4">
				<section
					class="overflow-hidden rounded-xl border bg-card"
					aria-labelledby="year-terms-heading"
				>
					<div class="border-b p-4">
						<h2 id="year-terms-heading" class="font-semibold">ภาคเรียนของปีนี้</h2>
						<p class="mt-1 text-sm text-muted-foreground">
							แยกการรวมผลรายปีออกจากเงื่อนไขปิดปี เช่น ภาคฤดูร้อนหรือภาคซ่อมเสริม
						</p>
					</div>
					<Table.Root
						><Table.Header
							><Table.Row
								><Table.Head class="min-w-40">ภาคเรียน</Table.Head><Table.Head class="min-w-24"
									>สถานะ</Table.Head
								><Table.Head class="min-w-28">รวมผลรายปี</Table.Head><Table.Head class="min-w-32"
									>ต้องปิดก่อนปิดปี</Table.Head
								><Table.Head><span class="sr-only">ตรวจภาคเรียน</span></Table.Head></Table.Row
							></Table.Header
						>
						<Table.Body
							>{#each workspace.terms as term (term.academicTermId)}<Table.Row
									><Table.Cell class="font-medium">{term.name}</Table.Cell><Table.Cell
										>{termStatusLabels[term.status]}</Table.Cell
									><Table.Cell>{term.includedInYearResult ? 'รวม' : 'ไม่รวม'}</Table.Cell
									><Table.Cell>{term.blocksYearClosure ? 'ต้องปิด' : 'ไม่บังคับ'}</Table.Cell
									><Table.Cell
										><Button
											variant="ghost"
											size="sm"
											href={resolve(
												`/staff/academic/term-lifecycle?academicYearId=${yearId}&academicTermId=${term.academicTermId}`
											)}>ตรวจภาคเรียน<ArrowUpRight class="size-4" /></Button
										></Table.Cell
									></Table.Row
								>{/each}</Table.Body
						>
					</Table.Root>
				</section>
				<section
					class="overflow-hidden rounded-xl border bg-card"
					aria-labelledby="annual-readiness-heading"
				>
					<div class="flex flex-wrap items-center justify-between gap-3 border-b p-4">
						<div>
							<h2 id="annual-readiness-heading" class="font-semibold">
								ผลรายปีและเรื่องที่ต้องตรวจ
							</h2>
							<p class="mt-1 text-sm text-muted-foreground">
								ผลล่าสุดครบ <span class="font-mono tabular-nums"
									>{readyCount} / {workspace.coverage.students.length}</span
								> คน
							</p>
						</div>
						{#if canReadAnnual}<Button
								variant="outline"
								size="sm"
								href={resolve(`/staff/academic/results/annual?academicYearId=${yearId}`)}
								>ตรวจผลรายปี<ArrowUpRight class="size-4" /></Button
							>{/if}
					</div>
					{#if workspace.findings.length === 0 && workspace.canClose}<p class="p-4 text-sm">
							ผลรายปีครบและภาคเรียนพร้อมปิดปีแล้ว
						</p>{:else}<ul class="divide-y">
							{#each workspace.findings as finding (finding.code)}<li
									class="flex flex-wrap items-start gap-3 p-4"
								>
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
								</li>{/each}
						</ul>{/if}
				</section>
			</div>
			<aside class="space-y-3" aria-label="การดำเนินการปีการศึกษา">
				<h2 class="font-semibold">ดำเนินการกับปีนี้</h2>
				<p class="text-sm text-muted-foreground">
					การปิดปีไม่เปลี่ยนคะแนน ผลที่ล็อก หรือห้องเรียนของนักเรียน การเตรียมปีใหม่ทำแยกกัน
				</p>
				{#each availableActions as candidate (candidate)}<Button
						class="w-full justify-start whitespace-normal text-start"
						variant={candidate === 'close' ? 'destructive' : 'outline'}
						disabled={busy || loading || (candidate === 'close' && !workspace.canClose)}
						onclick={() => openAction(candidate)}>{labels[candidate]}</Button
					>{/each}
				{#if workspace.context.status === 'closed'}
					{#key yearId}<YearReopeningDialog
							{yearId}
							yearName={workspace.context.name}
							disabled={busy || loading}
							onreopened={handleReopened}
						/>{/key}
				{:else if availableActions.length === 0}<p
						class="rounded-lg border border-dashed p-3 text-sm text-muted-foreground"
					>
						ดูข้อมูลได้ ไม่มีคำสั่งที่ใช้ได้ตามสถานะและสิทธิ์ของบัญชีนี้
					</p>{/if}
			</aside>
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={dialogOpen}>
	<Dialog.Content
		class="max-h-[85dvh] overflow-y-auto"
		showCloseButton={!busy}
		onInteractOutside={(event) => {
			if (busy) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (busy) event.preventDefault();
		}}
	>
		<Dialog.Header
			><Dialog.Title>{labels[action]}</Dialog.Title><Dialog.Description
				>{explanations[action]}</Dialog.Description
			></Dialog.Header
		>
		<p class="text-sm font-medium">{workspace?.context.name}</p>
		{#if action === 'close'}{#each warnings as warning (warning.code)}<div
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
				</div>{/each}{/if}
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
				>ยืนยัน{labels[action]}</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
