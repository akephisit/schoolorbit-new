<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';
	import { toast } from 'svelte-sonner';
	import { RefreshCw, ArrowUpRight } from '@lucide/svelte';
	import { registerAcademicContextDirtySource } from '#lib/academic-context/store.js';
	import { aggregateCapabilities } from '#lib/academic/results/aggregate-access.js';
	import {
		aggregateStatus,
		aggregateStatusLabels
	} from '#lib/academic/results/aggregate-presentation.js';

	import {
		listAnnualResultStudents,
		previewAnnualResult,
		listAnnualResultRevisions,
		lockAnnualResult,
		type AnnualResultStudent,
		type AnnualResultPreview,
		type AnnualResultRevision,
		type AnnualLockInput
	} from '#lib/api/academicAggregates.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import {
		PageState,
		PageSkeleton,
		LoadingButton,
		RegionUpdatingState
	} from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { can } from '#lib/stores/permissions.js';

	let { data }: PageProps = $props();
	const rosterRequest = new LatestRequest();
	const previewRequest = new LatestRequest();
	const historyRequest = new LatestRequest();
	let students = $state.raw<AnnualResultStudent[]>([]);
	let preview = $state.raw<AnnualResultPreview | null>(null);
	let history = $state.raw<AnnualResultRevision[]>([]);
	let selected = $state('');
	let search = $state('');
	let visibleCount = $state(50);
	let loading = $state(true);
	let rosterLoaded = $state(false);
	let previewLoading = $state(false);
	let historyLoading = $state(false);
	let previewLoaded = $state(false);
	let historyLoaded = $state(false);
	let error = $state('');
	let previewError = $state('');
	let historyError = $state('');
	let lockOpen = $state(false);
	let locking = $state(false);
	let holdReason = $state('');
	let lockError = $state('');
	let stale = $state(false);
	let pending: { key: string; body: AnnualLockInput } | null = null;
	const capabilities = $derived(aggregateCapabilities($can));
	const year = $derived(data.context?.academicYearId ?? null);
	const detailLoading = $derived(previewLoading || historyLoading);
	const student = $derived(students.find((row) => row.studentAcademicYearId === selected));
	const filtered = $derived(
		students.filter((row) =>
			`${row.studentCode ?? ''} ${row.studentName} ${row.gradeLevelName} ${row.studyProgramName}`
				.toLocaleLowerCase('th')
				.includes(search.trim().toLocaleLowerCase('th'))
		)
	);
	const confirmedCount = $derived(students.filter((row) => row.closure.isCurrent).length);
	const canConfirm = $derived(
		!locking &&
			!detailLoading &&
			!stale &&
			capabilities.lock &&
			!!preview?.canLock &&
			(preview.needsHold
				? holdReason.trim().length > 0 && Array.from(holdReason.trim()).length <= 1000
				: holdReason.trim() === '')
	);
	const resultLink = $derived(`/staff/academic/results${year ? `?academicYearId=${year}` : ''}`);

	function syncSelected(id: string): void {
		const url = new URL(page.url.href);

		if (id) url.searchParams.set('studentAcademicYearId', id);
		else url.searchParams.delete('studentAcademicYearId');

		goto(resolve(`staff/academic/results/annual?${url.searchParams.toString()}`), {
			shallow: true,
			replace: true,
			state: page.state
		});
	}

	async function loadRoster() {
		if (!year || !capabilities.read) return;
		const query = { academicYearId: year };
		const { revision, signal } = rosterRequest.begin();
		loading = true;
		error = '';
		try {
			const rows = await listAnnualResultStudents(query, { signal });
			if (!rosterRequest.isCurrent(revision)) return;
			students = rows;
			rosterLoaded = true;
			if (selected && !rows.some((row) => row.studentAcademicYearId === selected)) {
				previewRequest.abort();
				historyRequest.abort();
				selected = '';
				syncSelected('');
				preview = null;
				history = [];
			}
		} catch (cause) {
			if (!isAbortError(cause) && rosterRequest.isCurrent(revision))
				error = cause instanceof Error ? cause.message : 'โหลดรายชื่อไม่สำเร็จ';
		} finally {
			if (rosterRequest.isCurrent(revision)) loading = false;
		}
	}
	async function loadPreview(id: string): Promise<void> {
		if (!year || !capabilities.read) return;
		const query = { academicYearId: year };
		const { revision, signal } = previewRequest.begin();
		previewLoading = true;
		previewError = '';
		try {
			const calculated = await previewAnnualResult(id, query, { signal });
			if (!previewRequest.isCurrent(revision) || selected !== id || year !== query.academicYearId)
				return;
			preview = calculated;
			previewLoaded = true;
			stale = false;
		} catch (cause) {
			if (!isAbortError(cause) && previewRequest.isCurrent(revision))
				previewError = cause instanceof Error ? cause.message : 'ตรวจผลรายปีไม่สำเร็จ';
		} finally {
			if (previewRequest.isCurrent(revision)) previewLoading = false;
		}
	}
	async function loadHistory(id: string): Promise<void> {
		if (!year || !capabilities.read) return;
		const query = { academicYearId: year };
		const { revision, signal } = historyRequest.begin();
		historyLoading = true;
		historyError = '';
		try {
			const revisions = await listAnnualResultRevisions(id, query, { signal });
			if (!historyRequest.isCurrent(revision) || selected !== id || year !== query.academicYearId)
				return;
			history = revisions;
			historyLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && historyRequest.isCurrent(revision))
				historyError = cause instanceof Error ? cause.message : 'โหลดประวัติผลรายปีไม่สำเร็จ';
		} finally {
			if (historyRequest.isCurrent(revision)) historyLoading = false;
		}
	}
	async function inspect(id = selected) {
		if (!year || !capabilities.read || !id || locking) return;
		const changed = selected !== id;
		selected = id;
		syncSelected(id);
		if (changed) {
			preview = null;
			history = [];
			previewLoaded = false;
			historyLoaded = false;
		}
		await Promise.all([loadPreview(id), loadHistory(id)]);
	}
	async function refresh() {
		await Promise.all([loadRoster(), selected ? inspect() : Promise.resolve()]);
	}
	function openLock() {
		if (!capabilities.lock || !preview?.canLock || detailLoading || locking) return;
		holdReason = '';
		lockError = '';
		pending = null;
		lockOpen = true;
	}
	async function confirmLock() {
		if (!year || !preview || !selected || !canConfirm) return;
		const query = { academicYearId: year };
		const chosen = selected;
		const input = {
			expectedRevision: history[0]?.revision ?? null,
			sourceChecksum: preview.sourceChecksum,
			holdReason: preview.needsHold ? holdReason.trim() : null
		};
		const key = JSON.stringify([query, chosen, input]);
		if (pending?.key !== key) pending = { key, body: { ...input, requestId: crypto.randomUUID() } };
		locking = true;
		lockError = '';
		try {
			const locked = await lockAnnualResult(chosen, query, pending.body);
			if (year === query.academicYearId && selected === chosen) {
				rosterRequest.abort();
				historyRequest.abort();
				loading = false;
				historyLoading = false;
				historyLoaded = true;
				history = [
					locked,
					...history
						.filter((row) => row.id !== locked.id)
						.map((row) => ({ ...row, isCurrent: false }))
				];
				students = students.map((row) =>
					row.studentAcademicYearId === chosen
						? {
								...row,
								closure: {
									studentAcademicYearId: chosen,
									revisionId: locked.id,
									revision: locked.revision,
									isCurrent: locked.isCurrent,
									holdReason: locked.holdReason
								}
							}
						: row
				);
				lockOpen = false;
				toast.success('ยืนยันผลรายปีแล้ว');
				if (!rosterLoaded) void loadRoster();
			}
			pending = null;
		} catch (cause) {
			lockError = cause instanceof Error ? cause.message : 'ยืนยันผลรายปีไม่สำเร็จ';
			if (cause instanceof ApiClientError && cause.status === 409) stale = true;
		} finally {
			locking = false;
		}
	}

	onMount(() => registerAcademicContextDirtySource('annual-results', () => lockOpen || locking));
	$effect.pre(() => {
		const routeStudents = data.students;
		const routePreview = data.preview;
		const routeHistory = data.history;
		const routeSelectedId = data.selectedId;
		const rosterRevision = rosterRequest.begin().revision;
		const previewRevision = previewRequest.begin().revision;
		const historyRevision = historyRequest.begin().revision;
		untrack(() => {
			students = [];
			preview = null;
			history = [];
			selected = routeSelectedId ?? '';
			rosterLoaded = false;
			previewLoaded = false;
			historyLoaded = false;
			loading = Boolean(routeStudents);
			previewLoading = Boolean(routePreview);
			historyLoading = Boolean(routeHistory);
			error = '';
			previewError = '';
			historyError = '';
			lockOpen = false;
		});
		if (routeStudents)
			void routeStudents.then((result) => {
				if (!rosterRequest.isCurrent(rosterRevision)) return;
				untrack(() => {
					if (result.ok) {
						students = result.data;
						rosterLoaded = true;
					} else error = result.error;
					loading = false;
				});
			});
		if (routePreview)
			void routePreview.then((result) => {
				if (!previewRequest.isCurrent(previewRevision)) return;
				untrack(() => {
					if (result.ok) {
						preview = result.data;
						previewLoaded = true;
					} else previewError = result.error;
					previewLoading = false;
				});
			});
		if (routeHistory)
			void routeHistory.then((result) => {
				if (!historyRequest.isCurrent(historyRevision)) return;
				untrack(() => {
					if (result.ok) {
						history = result.data;
						historyLoaded = true;
					} else historyError = result.error;
					historyLoading = false;
				});
			});
		return () => {
			rosterRequest.abort();
			previewRequest.abort();
			historyRequest.abort();
		};
	});
</script>

<PageShell
	title="สรุปผลรายปี"
	description="ตรวจผลแต่ละภาคที่นำมารวม ก่อนยืนยันผลรายปีเพื่อปิดปีและพิจารณาเลื่อนชั้น"
	backHref={resultLink}
	backLabel="ผลรายวิชา"
>
	{#snippet actions()}
		<Button variant="outline" disabled={loading || locking} onclick={() => void refresh()}
			><RefreshCw class="size-4" />ตรวจข้อมูลล่าสุด</Button
		>
	{/snippet}
	{#if !capabilities.read}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์อ่านผลรวม"
			description="ต้องมีสิทธิ์อ่านผลการเรียนและผลประเมินระดับโรงเรียนทั้งสองส่วน"
		/>
	{:else if !year}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาก่อน"
			description="ใช้ตัวเลือกปีการศึกษาด้านบนเพื่อดูผลรายปี"
		/>
	{:else}
		<div class="grid items-start gap-4 xl:grid-cols-[minmax(280px,0.7fr)_minmax(0,1.7fr)]">
			{#if loading && !rosterLoaded}<PageSkeleton variant="table" rows={7} columns={2} />
			{:else if error && !rosterLoaded}<PageState
					variant="error"
					title="โหลดผลรายปีไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={() => void loadRoster()}
				/>
			{:else}<section
					class="overflow-hidden rounded-xl border bg-card"
					aria-label="นักเรียนในปีการศึกษา"
					aria-busy={loading}
				>
					<div class="space-y-3 border-b p-3 sm:p-4">
						{#if loading}<RegionUpdatingState
								class="static"
								label="กำลังอัปเดตรายชื่อนักเรียน..."
							/>{/if}
						{#if error}<div role="alert" class="flex items-center gap-2 text-sm text-destructive">
								<span>{error}</span><Button
									size="sm"
									variant="outline"
									onclick={() => void loadRoster()}>ลองใหม่</Button
								>
							</div>{/if}
						<div class="flex items-baseline justify-between gap-3">
							<h2 class="font-semibold">นักเรียน</h2>
							<span class="text-xs text-muted-foreground tabular-nums"
								>ยืนยันล่าสุด {confirmedCount} / {students.length} คน</span
							>
						</div>
						<Input
							aria-label="ค้นหานักเรียน"
							placeholder="รหัส ชื่อ ระดับชั้น หรือแผนการเรียน"
							bind:value={search}
							oninput={() => (visibleCount = 50)}
						/>
					</div>
					<div class="max-h-[36rem] overflow-y-auto divide-y">
						{#each filtered.slice(0, visibleCount) as row (row.studentAcademicYearId)}
							<button
								class={[
									'w-full space-y-1 px-4 py-3 text-left transition-colors hover:bg-muted/50 focus-visible:outline-2 focus-visible:outline-primary',
									selected === row.studentAcademicYearId && 'bg-primary/5'
								]}
								aria-label={row.studentName}
								aria-pressed={selected === row.studentAcademicYearId}
								disabled={locking}
								onclick={() => void inspect(row.studentAcademicYearId)}
							>
								<div class="flex items-baseline justify-between gap-3">
									<span class="text-sm font-medium">{row.studentName}</span><span
										class="text-xs text-muted-foreground tabular-nums"
										>{row.studentCode ?? '—'}</span
									>
								</div>
								<p class="text-xs text-muted-foreground">
									{row.gradeLevelName} · {row.studyProgramName}
								</p>
								<p
									class={[
										'text-xs',
										row.closure.isCurrent ? 'text-primary' : 'text-amber-700 dark:text-amber-400'
									]}
								>
									{aggregateStatusLabels[aggregateStatus(row.closure)]}{row.closure.holdReason
										? ' · มีผลค้างที่พิจารณาแล้ว'
										: ''}
								</p>
							</button>
						{:else}<p class="p-4 text-sm text-muted-foreground">ไม่พบนักเรียนในรายการนี้</p>{/each}
					</div>
					{#if filtered.length > visibleCount}<Button
							variant="ghost"
							class="w-full"
							onclick={() => (visibleCount += 50)}>แสดงเพิ่ม</Button
						>{/if}
				</section>{/if}
			<div class="min-w-0 space-y-4">
				{#if !selected}<PageState
						variant="empty"
						title="เลือกนักเรียนเพื่อตรวจผลรายปี"
						description="ระบบใช้ผลรายภาคที่ยืนยันแล้ว ไม่อ่านคะแนนที่ยังแก้ไขอยู่มาตัดสินเลื่อนชั้น"
					/>
				{:else}
					{#if previewLoading && !previewLoaded}<PageSkeleton
							variant="table"
							rows={5}
							columns={5}
						/>
					{:else if previewError && !previewLoaded}<PageState
							variant="error"
							title="ตรวจผลรายปีไม่สำเร็จ"
							description={previewError}
							actionLabel="ลองอีกครั้ง"
							onaction={() => void loadPreview(selected)}
						/>
					{:else if preview}<section
							class="overflow-hidden rounded-xl border bg-card"
							aria-busy={previewLoading}
						>
							{#if previewLoading}<RegionUpdatingState
									class="static m-3"
									label="กำลังคำนวณผลรายปีล่าสุด..."
								/>{/if}
							{#if previewError}<div
									role="alert"
									class="m-3 flex items-center gap-2 text-sm text-destructive"
								>
									<span>{previewError}</span><Button
										size="sm"
										variant="outline"
										onclick={() => void loadPreview(selected)}>ลองใหม่</Button
									>
								</div>{/if}
							<div class="flex flex-wrap items-center justify-between gap-3 border-b p-4">
								<div>
									<h2 class="font-semibold">
										{student?.studentName ?? 'ผลรายปีของนักเรียนที่เลือก'}
									</h2>
									<p class="mt-1 text-xs text-muted-foreground">
										ผลรายภาคที่นับรวมในปีนี้ · {preview.terms.length} ภาค
									</p>
								</div>
								{#if capabilities.lock}<Button
										disabled={!preview.canLock || locking || stale}
										onclick={openLock}>ยืนยันผลรายปี</Button
									>{/if}
							</div>
							<div class="overflow-x-auto">
								<Table.Root class="min-w-[660px] text-sm">
									<Table.Header
										><Table.Row
											><Table.Head>ภาคเรียน / รุ่นผล</Table.Head><Table.Head class="text-right"
												>หน่วยกิตเรียน</Table.Head
											><Table.Head class="text-right">หน่วยกิตผ่าน</Table.Head><Table.Head
												class="text-right">GPA ภาค</Table.Head
											><Table.Head>สถานะต้นทาง</Table.Head></Table.Row
										></Table.Header
									>
									<Table.Body
										>{#each preview.terms as source (source.academicTermId)}
											<Table.Row
												><Table.Cell
													><a
														class="inline-flex items-center gap-1 text-primary underline-offset-4 hover:underline"
														href={resolve(
															`staff/academic/results/aggregates?academicYearId=${year}&academicTermId=${source.academicTermId}&studentAcademicYearId=${selected}`
														)}>{source.termName}<ArrowUpRight class="size-3" /></a
													>
													<p class="mt-1 text-xs text-muted-foreground">
														{source.revision ? `รุ่น ${source.revision.revision}` : 'ยังไม่ยืนยัน'}
													</p></Table.Cell
												>
												<Table.Cell class="text-right tabular-nums"
													>{source.revision?.snapshot.results.totals.attemptedCredits ??
														'—'}</Table.Cell
												><Table.Cell class="text-right tabular-nums"
													>{source.revision?.snapshot.results.totals.earnedCredits ??
														'—'}</Table.Cell
												><Table.Cell class="text-right tabular-nums"
													>{source.revision?.officialGpa ?? '—'}</Table.Cell
												>
												<Table.Cell class="text-xs"
													>{!source.revision
														? 'ยังไม่มีผลรายภาคที่ยืนยันแล้ว'
														: !source.isCurrent
															? 'ต้นทางเปลี่ยน ต้องสรุปรายภาคใหม่'
															: source.revision.holdReason
																? 'ปัจจุบัน · มีผลค้าง'
																: 'ผลรายภาคปัจจุบัน'}</Table.Cell
												>
											</Table.Row>
										{:else}<Table.Row
												><Table.Cell colspan={5} class="py-6 text-center text-muted-foreground"
													>ยังไม่มีภาคเรียนที่นับรวมสำหรับนักเรียนคนนี้</Table.Cell
												></Table.Row
											>{/each}</Table.Body
									>
								</Table.Root>
							</div>
							<div class="grid gap-4 border-t bg-muted/20 p-4 sm:grid-cols-3">
								<div>
									<p class="text-xs text-muted-foreground">GPA รายปีที่ยืนยันล่าสุด</p>
									<p
										data-testid="annual-official-gpa"
										class="mt-1 text-xl font-semibold tabular-nums"
									>
										{historyLoaded ? (history[0]?.officialGpa ?? '—') : '…'}
									</p>
									<p class="mt-1 text-xs text-muted-foreground">
										{!historyLoaded
											? 'กำลังโหลดประวัติผลรายปี'
											: history[0]
												? history[0].isCurrent
													? history[0].holdReason
														? 'ยืนยันพร้อมผลค้าง'
														: 'ผลยืนยันเป็นปัจจุบัน'
													: 'ผลเดิมไม่เป็นปัจจุบัน'
												: 'ยังไม่ยืนยันผลรายปี'}
									</p>
								</div>
								<div>
									<p class="text-xs text-muted-foreground">ค่าเฉลี่ยตัวเลขจากผลรายภาค</p>
									<p class="mt-1 text-xl font-semibold tabular-nums">
										{preview.totals.provisionalGpa ?? '—'}
									</p>
									<p class="mt-1 text-xs text-muted-foreground">
										ผลรวมเกรดถ่วงน้ำหนัก ÷ หน่วยกิตที่มีเกรด
									</p>
								</div>
								<div>
									<p class="text-xs text-muted-foreground">หน่วยกิตที่ยังไม่มีเกรดตัวเลข</p>
									<p class="mt-1 text-xl font-semibold tabular-nums">
										{preview.totals.unresolvedCredits}
									</p>
									<p class="mt-1 text-xs text-muted-foreground">
										ผลพิเศษ {preview.totals.exceptionalResultCount} รายการ
									</p>
								</div>
							</div>
							<div class="space-y-1 border-t p-4 text-xs text-muted-foreground">
								{#if !preview.canLock}<p class="text-amber-700 dark:text-amber-400">
										ยังยืนยันรายปีไม่ได้ ให้ตรวจผลรายภาคที่ขาดหรือมีต้นทางเปลี่ยนก่อน
									</p>{/if}
								{#if preview.needsHold}<p>
										มีผลค้างที่ต้องพิจารณา การยืนยันจะเก็บเหตุผลและไม่สร้าง GPA ทางการแทนผลค้าง
									</p>{/if}
								<p>GPA รายปีไม่ใช่ GPAX · หน้านี้ยังไม่รวมผลสะสมข้ามปี</p>
							</div>
						</section>{/if}
					{#if historyLoading && !historyLoaded}<PageSkeleton
							variant="table"
							rows={3}
							columns={2}
						/>
					{:else if historyError && !historyLoaded}<PageState
							variant="error"
							title="โหลดประวัติผลรายปีไม่สำเร็จ"
							description={historyError}
							actionLabel="ลองอีกครั้ง"
							onaction={() => void loadHistory(selected)}
						/>
					{:else}<section
							class="overflow-hidden rounded-xl border bg-card"
							aria-label="ประวัติผลรายปี"
							aria-busy={historyLoading}
						>
							<h2 class="border-b px-4 py-3 text-sm font-semibold">ประวัติการยืนยันผลรายปี</h2>
							{#if historyLoading}<RegionUpdatingState
									class="static m-3"
									label="กำลังอัปเดตประวัติผลรายปี..."
								/>{/if}
							{#if historyError}<div
									role="alert"
									class="m-3 flex items-center gap-2 text-sm text-destructive"
								>
									<span>{historyError}</span><Button
										size="sm"
										variant="outline"
										onclick={() => void loadHistory(selected)}>ลองใหม่</Button
									>
								</div>{/if}
							{#each history as revision (revision.id)}<div
									class="space-y-1 border-b px-4 py-3 text-sm last:border-b-0"
								>
									<div class="flex flex-wrap items-center justify-between gap-2">
										<span
											>รุ่น {revision.revision} · {revision.isCurrent
												? 'เป็นปัจจุบัน'
												: 'ประวัติเดิม'}</span
										><span class="tabular-nums">GPA {revision.officialGpa ?? '—'}</span>
									</div>
									<p class="text-xs text-muted-foreground">
										{new Date(revision.lockedAt).toLocaleString('th-TH')}{revision.holdReason
											? ` · ${revision.holdReason}`
											: ''}
									</p>
								</div>
							{:else}<p class="p-4 text-sm text-muted-foreground">
									ยังไม่มีประวัติการยืนยัน
								</p>{/each}
						</section>{/if}
				{/if}
			</div>
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={lockOpen}>
	<Dialog.Content
		class="sm:max-w-lg"
		showCloseButton={!locking}
		onInteractOutside={(event) => {
			if (locking) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (locking) event.preventDefault();
		}}
	>
		<Dialog.Header
			><Dialog.Title>ยืนยันผลรายปี</Dialog.Title><Dialog.Description
				>เก็บผลรายภาครุ่นที่ตรวจแล้วเป็นผลรายปีรุ่นใหม่
				ไม่แก้คะแนนหรือเลื่อนชั้นนักเรียนโดยอัตโนมัติ</Dialog.Description
			></Dialog.Header
		>
		<p class="text-sm font-medium">
			{student?.studentName} · {preview?.terms.length ?? 0} ภาคเรียน
		</p>
		{#if preview?.needsHold}<div class="space-y-2">
				<Label for="annual-hold-reason">เหตุผลที่พิจารณาผลค้าง</Label><Textarea
					id="annual-hold-reason"
					bind:value={holdReason}
					maxlength={1000}
					disabled={locking}
				/>
				<p class="text-xs text-muted-foreground">เก็บผลค้างไว้โดยไม่มี GPA ทางการ</p>
			</div>{/if}
		{#if lockError}<p role="alert" class="text-sm text-destructive">{lockError}</p>{/if}
		{#if stale}<Button
				variant="outline"
				onclick={() => {
					lockOpen = false;
					void inspect();
				}}>กลับไปตรวจผลล่าสุด</Button
			>{/if}
		<Dialog.Footer
			><Button variant="outline" disabled={locking} onclick={() => (lockOpen = false)}
				>ยกเลิก</Button
			><LoadingButton loading={locking} disabled={!canConfirm} onclick={() => void confirmLock()}
				>ยืนยันผลรายปี</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
