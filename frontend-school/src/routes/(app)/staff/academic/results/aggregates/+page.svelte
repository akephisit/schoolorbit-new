<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { page } from '$app/state';
	import { replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';
	import { toast } from 'svelte-sonner';
	import { RefreshCw, Settings2 } from '@lucide/svelte';
	import { registerAcademicContextDirtySource } from '$lib/academic-context/store';
	import { aggregateCapabilities } from '$lib/academic/results/aggregate-access';
	import {
		aggregateStatus,
		aggregateStatusLabels,
		aggregateBlockerLabels,
		aggregateHoldLabels,
		canLockAggregate
	} from '$lib/academic/results/aggregate-presentation';
	import {
		listAggregateStudents,
		listAggregatePolicies,
		createAggregatePolicy,
		previewTermAggregate,
		listTermAggregateRevisions,
		lockTermAggregate,
		type AggregateStudent,
		type AggregatePolicyVersion,
		type TermAggregatePreview,
		type TermAggregateRevision,
		type AggregateLockInput
	} from '$lib/api/academicAggregates';
	import { ApiClientError } from '$lib/api/client';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { PageShell } from '$lib/components/app-layout';
	import {
		PageState,
		PageSkeleton,
		LoadingButton,
		RegionUpdatingState
	} from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import { can } from '$lib/stores/permissions';

	let { data }: PageProps = $props();
	const rosterRequest = new LatestRequest();
	const policyRequest = new LatestRequest();
	const previewRequest = new LatestRequest();
	const historyRequest = new LatestRequest();
	let students = $state.raw<AggregateStudent[]>([]);
	let policies = $state.raw<AggregatePolicyVersion[]>([]);
	let preview = $state.raw<TermAggregatePreview | null>(null);
	let history = $state.raw<TermAggregateRevision[]>([]);
	let selected = $state('');
	let policyId = $state('');
	let search = $state('');
	let visibleCount = $state(50);
	let loading = $state(true);
	let rosterLoaded = $state(false);
	let policiesLoading = $state(true);
	let policiesLoaded = $state(false);
	let previewLoading = $state(false);
	let previewLoaded = $state(false);
	let historyLoading = $state(false);
	let historyLoaded = $state(false);
	let error = $state('');
	let policyLoadError = $state('');
	let previewError = $state('');
	let historyError = $state('');
	let lockOpen = $state(false);
	let policyOpen = $state(false);
	let locking = $state(false);
	let savingPolicy = $state(false);
	let holdReason = $state('');
	let lockError = $state('');
	let stale = $state(false);
	let policyName = $state('');
	let passingGrade = $state('1');
	let minimumLevel = $state('1');
	let allowHolds = $state(false);
	let reviewedPolicy = $state(false);
	let policyError = $state('');
	let pending: { key: string; body: AggregateLockInput } | null = null;
	const capabilities = $derived(aggregateCapabilities($can));
	const year = $derived(data.context?.academicYearId ?? null);
	const term = $derived(data.context?.academicTermId ?? null);
	const detailLoading = $derived(previewLoading || historyLoading);
	const student = $derived(students.find((row) => row.studentAcademicYearId === selected));
	const filtered = $derived(
		students.filter((row) =>
			`${row.studentCode ?? ''} ${row.studentName} ${row.gradeLevelName} ${row.studyProgramName}`
				.toLocaleLowerCase('th')
				.includes(search.trim().toLocaleLowerCase('th'))
		)
	);
	const activePolicy = $derived(policies.find((row) => row.id === policyId));
	const canConfirm = $derived(
		!locking && !detailLoading && canLockAggregate(preview, holdReason, capabilities.lock, stale)
	);
	const contextQuery = $derived(
		year && term ? { academicYearId: year, academicTermId: term } : null
	);
	const resultLink = $derived(
		contextQuery
			? `/staff/academic/results?academicYearId=${year}&academicTermId=${term}`
			: '/staff/academic/results'
	);

	function syncSelection(): void {
		const url = new URL(page.url);
		if (selected) url.searchParams.set('studentAcademicYearId', selected);
		else url.searchParams.delete('studentAcademicYearId');
		if (policyId) url.searchParams.set('policyId', policyId);
		else url.searchParams.delete('policyId');
		replaceState(
			resolve(`/staff/academic/results/aggregates?${url.searchParams.toString()}`),
			page.state
		);
	}

	async function loadRoster() {
		if (!contextQuery || !capabilities.read) return;
		const query = contextQuery;
		const { revision, signal } = rosterRequest.begin();
		loading = true;
		error = '';
		try {
			const rows = await listAggregateStudents(query, { signal });
			if (
				!rosterRequest.isCurrent(revision) ||
				year !== query.academicYearId ||
				term !== query.academicTermId
			)
				return;
			students = rows;
			rosterLoaded = true;
			if (selected && !rows.some((row) => row.studentAcademicYearId === selected)) {
				previewRequest.abort();
				historyRequest.abort();
				selected = '';
				preview = null;
				history = [];
				syncSelection();
			}
		} catch (cause) {
			if (!isAbortError(cause) && rosterRequest.isCurrent(revision))
				error = cause instanceof Error ? cause.message : 'โหลดข้อมูลไม่สำเร็จ';
		} finally {
			if (rosterRequest.isCurrent(revision)) loading = false;
		}
	}
	async function loadPolicies(): Promise<void> {
		if (!capabilities.read) return;
		const { revision, signal } = policyRequest.begin();
		policiesLoading = true;
		policyLoadError = '';
		try {
			const versions = await listAggregatePolicies({ signal });
			if (!policyRequest.isCurrent(revision)) return;
			policies = versions;
			policiesLoaded = true;
			if (!versions.some((version) => version.id === policyId)) {
				policyId = versions[0]?.id ?? '';
				syncSelection();
				if (selected && policyId) void loadPreview(selected, policyId);
			}
		} catch (cause) {
			if (!isAbortError(cause) && policyRequest.isCurrent(revision))
				policyLoadError = cause instanceof Error ? cause.message : 'โหลดนโยบายไม่สำเร็จ';
		} finally {
			if (policyRequest.isCurrent(revision)) policiesLoading = false;
		}
	}
	async function loadHistory(id: string): Promise<void> {
		const query = contextQuery;
		if (!query || !capabilities.read) return;
		const { revision, signal } = historyRequest.begin();
		historyLoading = true;
		historyError = '';
		try {
			const revisions = await listTermAggregateRevisions(id, query, { signal });
			if (
				!historyRequest.isCurrent(revision) ||
				selected !== id ||
				year !== query.academicYearId ||
				term !== query.academicTermId
			)
				return;
			history = revisions;
			historyLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && historyRequest.isCurrent(revision))
				historyError = cause instanceof Error ? cause.message : 'โหลดประวัติผลสรุปไม่สำเร็จ';
		} finally {
			if (historyRequest.isCurrent(revision)) historyLoading = false;
		}
	}
	async function loadPreview(id: string, chosenPolicyId = policyId): Promise<void> {
		const query = contextQuery;
		if (!query || !capabilities.read || !chosenPolicyId) return;
		const { revision, signal } = previewRequest.begin();
		previewLoading = true;
		previewError = '';
		try {
			const calculated = await previewTermAggregate(id, query, chosenPolicyId, { signal });
			if (
				!previewRequest.isCurrent(revision) ||
				selected !== id ||
				policyId !== chosenPolicyId ||
				year !== query.academicYearId ||
				term !== query.academicTermId
			)
				return;
			preview = calculated;
			previewLoaded = true;
			stale = false;
		} catch (cause) {
			if (!isAbortError(cause) && previewRequest.isCurrent(revision))
				previewError = cause instanceof Error ? cause.message : 'ตรวจผลไม่สำเร็จ';
		} finally {
			if (previewRequest.isCurrent(revision)) previewLoading = false;
		}
	}
	async function inspect(id = selected) {
		if (!contextQuery || !capabilities.read || !id) return;
		const changed = selected !== id;
		selected = id;
		syncSelection();
		if (changed) {
			preview = null;
			history = [];
			previewLoaded = false;
			historyLoaded = false;
		}
		await Promise.all([loadHistory(id), policyId ? loadPreview(id) : Promise.resolve()]);
	}
	function openLock() {
		if (!preview || !capabilities.lock || !preview.canLock || detailLoading) return;
		holdReason = '';
		lockError = '';
		pending = null;
		lockOpen = true;
	}
	async function confirmLock() {
		if (!contextQuery || !preview || !selected || !canConfirm) return;
		const chosen = selected;
		const query = contextQuery;
		const input = {
			policyId: preview.policy.id,
			sourceChecksum: preview.sourceChecksum,
			expectedRevision: history[0]?.revision ?? null,
			holdReason: preview.holdFindings.length ? holdReason.trim() : null
		};
		const key = JSON.stringify([query, chosen, input]);
		if (pending?.key !== key) pending = { key, body: { ...input, requestId: crypto.randomUUID() } };
		locking = true;
		lockError = '';
		try {
			const locked = await lockTermAggregate(chosen, query, pending.body);
			if (selected === chosen && year === query.academicYearId && term === query.academicTermId) {
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
									...row.closure,
									revisionId: locked.id,
									revision: locked.revision,
									policyId: locked.snapshot.policy.id,
									isCurrent: locked.isCurrent,
									blockers: locked.snapshot.blockers,
									holdReason: locked.holdReason,
									currentSourceChecksum: locked.snapshot.sourceChecksum
								}
							}
						: row
				);
				lockOpen = false;
				toast.success('ล็อกผลสรุปแล้ว');
				if (!rosterLoaded) void loadRoster();
			}
			pending = null;
		} catch (cause) {
			lockError = cause instanceof Error ? cause.message : 'ล็อกผลสรุปไม่สำเร็จ';
			if (cause instanceof ApiClientError && cause.status === 409) stale = true;
		} finally {
			locking = false;
		}
	}
	async function savePolicy() {
		if (!capabilities.policy || !reviewedPolicy || !policyName.trim() || savingPolicy) return;
		savingPolicy = true;
		policyError = '';
		try {
			const created = await createAggregatePolicy({
				name: policyName.trim(),
				passingGrade,
				minimumLearnerLevel: Number(minimumLevel),
				allowReviewedHolds: allowHolds
			});
			policies = [created, ...policies];
			policyId = created.id;
			syncSelection();
			policyOpen = false;
			reviewedPolicy = false;
			toast.success('บันทึกนโยบายที่ตรวจแล้ว');
			if (selected) await loadPreview(selected, created.id);
		} catch (cause) {
			policyError = cause instanceof Error ? cause.message : 'บันทึกนโยบายไม่สำเร็จ';
		} finally {
			savingPolicy = false;
		}
	}
	onMount(() =>
		registerAcademicContextDirtySource(
			'term-aggregate',
			() => lockOpen || policyOpen || locking || savingPolicy
		)
	);
	$effect.pre(() => {
		const routeStudents = data.students;
		const routePolicies = data.policies;
		const routePreview = data.preview;
		const routeHistory = data.history;
		const routeSelectedId = data.selectedId;
		const requestedPolicyId = data.requestedPolicyId;
		const rosterRevision = rosterRequest.begin().revision;
		const policyRevision = policyRequest.begin().revision;
		const previewRevision = previewRequest.begin().revision;
		const historyRevision = historyRequest.begin().revision;
		untrack(() => {
			students = [];
			policies = [];
			preview = null;
			history = [];
			selected = routeSelectedId ?? '';
			policyId = requestedPolicyId ?? '';
			rosterLoaded = false;
			policiesLoaded = false;
			previewLoaded = false;
			historyLoaded = false;
			loading = Boolean(routeStudents);
			policiesLoading = Boolean(routePolicies);
			previewLoading = Boolean(routePreview);
			historyLoading = Boolean(routeHistory);
			error = '';
			policyLoadError = '';
			previewError = '';
			historyError = '';
			lockOpen = false;
			policyOpen = false;
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
		if (routePolicies)
			void routePolicies.then((result) => {
				if (!policyRequest.isCurrent(policyRevision)) return;
				untrack(() => {
					if (result.ok) {
						policies = result.data;
						policiesLoaded = true;
						if (!result.data.some((version) => version.id === policyId)) {
							policyId = result.data[0]?.id ?? '';
							syncSelection();
							if (requestedPolicyId) {
								previewRequest.abort();
								preview = null;
								previewLoaded = false;
								previewLoading = false;
								if (selected && policyId) void loadPreview(selected, policyId);
							}
						}
					} else policyLoadError = result.error;
					policiesLoading = false;
				});
			});
		if (routePreview)
			void routePreview.then((result) => {
				if (!previewRequest.isCurrent(previewRevision)) return;
				untrack(() => {
					if (result.policyId) policyId = result.policyId;
					if (result.result?.ok) {
						preview = result.result.data;
						previewLoaded = true;
					} else if (result.result && !result.result.ok) previewError = result.result.error;
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
			policyRequest.abort();
			previewRequest.abort();
			historyRequest.abort();
		};
	});
</script>

<PageShell
	title="สรุปผลรายภาค"
	description="ตรวจผลรายวิชา กิจกรรม และผลประเมินของนักเรียนก่อนล็อกผลสรุปเพื่อปิดภาคเรียน"
	backHref={resultLink}
	backLabel="ผลรายวิชา"
>
	{#snippet actions()}
		<Button
			variant="outline"
			disabled={loading || locking || savingPolicy}
			onclick={() => void loadRoster()}><RefreshCw class="size-4" />ตรวจรายชื่อล่าสุด</Button
		>
		{#if capabilities.policy}<Button
				variant="outline"
				onclick={() => {
					policyOpen = true;
					reviewedPolicy = false;
					policyError = '';
				}}><Settings2 class="size-4" />นโยบายผลสรุป</Button
			>{/if}
	{/snippet}
	{#if !capabilities.read}<PageState
			variant="permission"
			title="ต้องมีสิทธิ์ดูผลการเรียนและผลประเมินระดับโรงเรียนทั้งสองส่วน"
		/>
	{:else if !contextQuery}<PageState title="เลือกปีการศึกษาและภาคเรียนก่อน" />
	{:else}
		<div class="flex flex-wrap items-end gap-3 rounded-xl border bg-card p-3 sm:p-4">
			<div class="min-w-56 flex-1 space-y-2">
				<Label for="aggregate-search">ค้นหานักเรียนหรือระดับชั้น</Label><Input
					id="aggregate-search"
					bind:value={search}
					placeholder="ชื่อ เลขประจำตัว ระดับชั้น หรือแผนการเรียน"
				/>
			</div>
			<div class="min-w-64 flex-1 space-y-2">
				<Label for="aggregate-policy">นโยบายที่ใช้คำนวณ</Label><Select.Root
					type="single"
					value={policyId}
					onValueChange={(value) => {
						policyId = value;
						syncSelection();
						preview = null;
						previewLoaded = false;
						if (selected) void loadPreview(selected, value);
					}}
					disabled={locking || policiesLoading}
					><Select.Trigger id="aggregate-policy" class="w-full"
						>{activePolicy?.name ?? 'ยังไม่มีนโยบายที่ตรวจแล้ว'}</Select.Trigger
					><Select.Content
						>{#each policies as policy (policy.id)}<Select.Item value={policy.id}
								>{policy.name}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
				{#if policiesLoading && !policiesLoaded}<PageSkeleton variant="form" rows={1} />{/if}
				{#if policyLoadError}<div
						role="alert"
						class="flex items-center gap-2 text-sm text-destructive"
					>
						<span>{policyLoadError}</span><Button
							size="sm"
							variant="outline"
							onclick={() => void loadPolicies()}>ลองใหม่</Button
						>
					</div>{/if}
			</div>
		</div>
		{#if policiesLoaded && policies.length === 0}<PageState
				title="ยังไม่มีนโยบายผลสรุปที่ตรวจแล้ว"
				description="ฝ่ายวิชาการต้องตรวจเกณฑ์ผ่าน ผลประเมินขั้นต่ำ และการยอมรับผลค้างก่อน จึงจะคำนวณและล็อกผลสรุปได้"
			/>{/if}
		<div class="grid items-start gap-5 xl:grid-cols-[minmax(22rem,0.9fr)_minmax(0,1.1fr)]">
			{#if loading && !rosterLoaded}<PageSkeleton variant="table" rows={7} columns={2} />
			{:else if error && !rosterLoaded}<PageState
					variant="error"
					title="โหลดผลสรุปไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={() => void loadRoster()}
				/>
			{:else}<section
					class="overflow-hidden rounded-xl border bg-card"
					aria-label="รายชื่อนักเรียน"
					aria-busy={loading}
				>
					<div class="border-b p-4 text-sm">
						{#if loading}<RegionUpdatingState
								class="static mb-2"
								label="กำลังอัปเดตรายชื่อนักเรียน..."
							/>{/if}
						{#if error}<div role="alert" class="mb-2 flex items-center gap-2 text-destructive">
								<span>{error}</span><Button
									size="sm"
									variant="outline"
									onclick={() => void loadRoster()}>ลองใหม่</Button
								>
							</div>{/if}
						{filtered.length} คน · ผลสรุปล่าสุด {students.filter((row) => row.closure.isCurrent)
							.length} / {students.length} คน
					</div>
					<Table.Root
						><Table.Header
							><Table.Row
								><Table.Head>นักเรียน</Table.Head><Table.Head>สถานะผลสรุป</Table.Head></Table.Row
							></Table.Header
						><Table.Body>
							{#each filtered.slice(0, visibleCount) as row (row.studentAcademicYearId)}
								<Table.Row class={selected === row.studentAcademicYearId ? 'bg-primary/5' : ''}
									><Table.Cell
										><Button
											variant="link"
											class="h-auto p-0 text-start"
											disabled={locking}
											aria-label={`ตรวจผล ${row.studentName}`}
											onclick={() => void inspect(row.studentAcademicYearId)}
											>{row.studentName}</Button
										>
										<p class="mt-1 text-xs text-muted-foreground">
											{row.studentCode ?? 'ยังไม่มีเลขประจำตัว'} · {row.gradeLevelName}
										</p>
										<p class="text-xs text-muted-foreground">{row.studyProgramName}</p></Table.Cell
									><Table.Cell
										><span class={row.closure.isCurrent ? 'text-emerald-700' : 'text-amber-800'}
											>{aggregateStatusLabels[aggregateStatus(row.closure)]}</span
										></Table.Cell
									></Table.Row
								>
							{/each}
						</Table.Body></Table.Root
					>
					{#if filtered.length === 0}<p class="p-5 text-sm text-muted-foreground">
							ไม่พบนักเรียนในรายการที่เลือก
						</p>{/if}
					{#if filtered.length > visibleCount}<div class="border-t p-3">
							<Button variant="ghost" onclick={() => (visibleCount += 50)}>แสดงเพิ่ม 50 คน</Button>
						</div>{/if}
				</section>{/if}
			<section class="space-y-4 rounded-xl border bg-card p-4 sm:p-5" aria-label="รายละเอียดผลสรุป">
				{#if !selected}<PageState
						title="เลือกนักเรียนเพื่อตรวจผล"
						description="รายการนี้รวมคนที่ยังไม่มีผลสรุปด้วย การเปิดดูยังไม่ล็อกผล"
					/>
				{:else}
					<div class="flex items-start justify-between gap-3">
						<div>
							<h2 class="font-semibold">{student?.studentName ?? 'ผลสรุปของนักเรียนที่เลือก'}</h2>
							<p class="mt-1 text-sm text-muted-foreground">
								{student
									? `${student.gradeLevelName} · ${student.studyProgramName}`
									: 'กำลังโหลดข้อมูลนักเรียน'}
							</p>
						</div>
						<Button size="sm" variant="outline" disabled={locking} onclick={() => void inspect()}
							>คำนวณใหม่</Button
						>
					</div>
					{#if previewLoading && !previewLoaded}<PageSkeleton
							variant="table"
							rows={3}
							columns={2}
						/>
					{:else if previewError && !previewLoaded}<PageState
							variant="error"
							title="ตรวจผลไม่สำเร็จ"
							description={previewError}
							actionLabel="ลองอีกครั้ง"
							onaction={() => void loadPreview(selected)}
						/>
					{:else if preview}
						{#if previewLoading}<RegionUpdatingState
								class="static"
								label="กำลังคำนวณผลสรุปล่าสุด..."
							/>{/if}
						{#if previewError}<div
								role="alert"
								class="flex items-center gap-2 text-sm text-destructive"
							>
								<span>{previewError}</span><Button
									size="sm"
									variant="outline"
									onclick={() => void loadPreview(selected)}>ลองใหม่</Button
								>
							</div>{/if}
						<div class="rounded-lg border bg-muted/30 p-3 text-sm">
							<p>นโยบาย: {preview.policy.name}</p>
							<p class="mt-1 text-muted-foreground">
								เกรดผ่านขั้นต่ำ {preview.policy.passingGrade} · ผลประเมินขั้นต่ำระดับ {preview
									.policy.minimumLearnerLevel}
							</p>
						</div>
						<dl class="grid grid-cols-[1fr_auto] gap-x-4 gap-y-2 text-sm">
							<dt>หน่วยกิตที่เรียน</dt>
							<dd class="font-mono tabular-nums">{preview.results.totals.attemptedCredits}</dd>
							<dt>หน่วยกิตที่มีเกรดตัวเลข</dt>
							<dd class="font-mono tabular-nums">{preview.results.totals.gradedCredits}</dd>
							<dt>หน่วยกิตที่ผ่าน</dt>
							<dd class="font-mono tabular-nums">{preview.results.totals.earnedCredits}</dd>
							<dt>หน่วยกิตที่ยังมีผลค้าง</dt>
							<dd class="font-mono tabular-nums">{preview.results.totals.unresolvedCredits}</dd>
							<dt>ผลรวมเกรด × หน่วยกิต</dt>
							<dd class="font-mono tabular-nums">{preview.results.totals.weightedGradePoints}</dd>
							<dt class="border-t pt-2 font-medium">ค่าเฉลี่ยเฉพาะผลตัวเลข</dt>
							<dd class="border-t pt-2 font-mono font-semibold tabular-nums">
								{preview.results.totals.provisionalGpa ?? '—'}
							</dd>
						</dl>
						<p class="text-xs text-muted-foreground">
							ค่าเฉลี่ยนี้ยังไม่ใช่ GPA ทางการจนกว่าจะล็อกผลสรุปที่ครบและไม่มีผลค้าง ค่า 0
							เป็นผลตัวเลข ไม่ใช่ช่องว่าง
						</p>
						<div class="border-t pt-3 text-sm">
							<p>
								กิจกรรมผ่าน {preview.results.activityTotals.passedGroupCount} / {preview.results
									.activityTotals.expectedGroupCount} กลุ่ม · ยังขาดผล {preview.results
									.activityTotals.missingResultCount} กลุ่ม
							</p>
							{#each preview.learnerEvaluations.domains as domain (domain.domain)}<p class="mt-2">
									{domain.domain === 'desirable_characteristic'
										? 'คุณลักษณะอันพึงประสงค์'
										: 'การอ่าน คิดวิเคราะห์ และเขียน'}: {domain.complete
										? `ระดับ ${domain.qualityLevel ?? '—'}`
										: `ยังขาดผล ${domain.missingSubjects.length} วิชา`}
								</p>{/each}
						</div>
						{#if preview.blockers.length || preview.holdFindings.length}<ul
								class="space-y-2 rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-950"
							>
								{#each preview.blockers as blocker (blocker)}<li>
										{aggregateBlockerLabels[blocker]}
									</li>{/each}{#each preview.holdFindings as finding (finding)}<li>
										{aggregateHoldLabels[finding]} · ต้องตรวจและระบุเหตุผล
									</li>{/each}
							</ul>{/if}
						<div class="flex flex-wrap gap-2">
							<Button variant="outline" href={resultLink}>ตรวจผลต้นทาง</Button
							>{#if capabilities.lock}<Button
									disabled={!preview.canLock || stale || locking}
									onclick={openLock}>ล็อกผลสรุป</Button
								>{/if}
						</div>
					{/if}
					{#if historyLoading && !historyLoaded}<PageSkeleton
							variant="table"
							rows={3}
							columns={2}
						/>
					{:else if historyError && !historyLoaded}<PageState
							variant="error"
							title="โหลดประวัติผลสรุปไม่สำเร็จ"
							description={historyError}
							actionLabel="ลองอีกครั้ง"
							onaction={() => void loadHistory(selected)}
						/>
					{:else if historyLoaded}<div class="space-y-3 border-t pt-4" aria-busy={historyLoading}>
							<h3 class="text-sm font-semibold">ประวัติผลสรุปที่ล็อกแล้ว</h3>
							{#if historyLoading}<RegionUpdatingState
									class="static"
									label="กำลังอัปเดตประวัติผลสรุป..."
								/>{/if}
							{#if historyError}<div
									role="alert"
									class="flex items-center gap-2 text-sm text-destructive"
								>
									<span>{historyError}</span><Button
										size="sm"
										variant="outline"
										onclick={() => void loadHistory(selected)}>ลองใหม่</Button
									>
								</div>{/if}
							{#if history.length === 0}<p class="text-sm text-muted-foreground">
									ยังไม่มีประวัติผลสรุปที่ล็อกแล้ว
								</p>{/if}
							{#each history as revision (revision.id)}<div class="rounded-lg border p-3 text-sm">
									<div class="flex flex-wrap justify-between gap-2">
										<span
											>รุ่น {revision.revision} · {revision.isCurrent
												? 'ข้อมูลต้นทางยังตรงกัน'
												: 'ประวัติเดิม / ต้นทางเปลี่ยน'}</span
										><span class="text-muted-foreground"
											>{new Date(revision.lockedAt).toLocaleDateString('th-TH')}</span
										>
									</div>
									<p class="mt-2 font-medium">GPA ทางการ: {revision.officialGpa ?? 'ยังไม่สรุป'}</p>
									{#if revision.holdReason}<p class="mt-1 text-amber-800">
											ติดตามผลค้าง: {revision.holdReason}
										</p>{/if}
									<p class="mt-1 text-xs text-muted-foreground">
										นโยบาย: {revision.snapshot.policy.name}
									</p>
								</div>{/each}
						</div>{/if}
				{/if}
			</section>
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={lockOpen}
	><Dialog.Content class="max-h-[85dvh] overflow-y-auto"
		><Dialog.Header
			><Dialog.Title>ล็อกผลสรุปของ {student?.studentName}</Dialog.Title><Dialog.Description
				>บันทึกผลสรุปรุ่นใหม่จากข้อมูลที่ตรวจล่าสุด ประวัติเดิมยังอยู่
				ไม่แก้คะแนนรายวิชาหรือคะแนนย่อย</Dialog.Description
			></Dialog.Header
		>
		{#if preview?.holdFindings.length}<div class="space-y-2">
				<Label for="aggregate-hold">เหตุผลที่รับทราบและค้างผลไว้</Label><Textarea
					id="aggregate-hold"
					bind:value={holdReason}
					maxlength={1000}
					disabled={locking}
				/>
				<p class="text-xs text-muted-foreground">
					ผลสรุปนี้จะยังไม่มี GPA ทางการ ต้องติดตามผลค้างต่อ
				</p>
			</div>{/if}
		{#if lockError}<p role="alert" class="text-sm text-destructive">{lockError}</p>{/if}
		{#if stale}<LoadingButton
				variant="outline"
				loading={detailLoading}
				onclick={() => void inspect()}>คำนวณข้อมูลล่าสุด</LoadingButton
			>{/if}
		<Dialog.Footer
			><Button variant="outline" disabled={locking} onclick={() => (lockOpen = false)}
				>กลับไปตรวจสอบ</Button
			><LoadingButton loading={locking} disabled={!canConfirm} onclick={() => void confirmLock()}
				>ยืนยันล็อกผลสรุป</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content></Dialog.Root
>

<Dialog.Root bind:open={policyOpen}
	><Dialog.Content class="max-h-[85dvh] overflow-y-auto"
		><Dialog.Header
			><Dialog.Title>นโยบายผลสรุปที่โรงเรียนตรวจแล้ว</Dialog.Title><Dialog.Description
				>สร้างเป็นรุ่นใหม่ ไม่เปลี่ยนนโยบายในประวัติที่ล็อกแล้ว และไม่ใช่เกณฑ์ตัดเกรดรายวิชา</Dialog.Description
			></Dialog.Header
		>
		<div class="space-y-2">
			<Label for="aggregate-policy-name">ชื่อนโยบาย</Label><Input
				id="aggregate-policy-name"
				bind:value={policyName}
				maxlength={160}
				disabled={savingPolicy}
			/>
		</div>
		<div class="space-y-2">
			<Label for="aggregate-passing-grade">เกรดขั้นต่ำที่นับหน่วยกิตผ่าน</Label><Input
				id="aggregate-passing-grade"
				bind:value={passingGrade}
				inputmode="decimal"
				disabled={savingPolicy}
			/>
		</div>
		<div class="space-y-2">
			<Label for="aggregate-minimum-level">ผลประเมินขั้นต่ำ</Label><Select.Root
				type="single"
				bind:value={minimumLevel}
				disabled={savingPolicy}
				><Select.Trigger id="aggregate-minimum-level">ระดับ {minimumLevel}</Select.Trigger
				><Select.Content
					>{#each ['0', '1', '2', '3'] as level (level)}<Select.Item value={level}
							>ระดับ {level}</Select.Item
						>{/each}</Select.Content
				></Select.Root
			>
		</div>
		<div class="flex items-start gap-3">
			<Checkbox id="aggregate-allow-hold" bind:checked={allowHolds} disabled={savingPolicy} /><Label
				for="aggregate-allow-hold"
				class="leading-relaxed">อนุญาตให้ล็อกพร้อมผลค้างที่ตรวจแล้ว โดยต้องระบุเหตุผลรายคน</Label
			>
		</div>
		<div class="flex items-start gap-3 rounded-lg border p-3">
			<Checkbox
				id="aggregate-policy-reviewed"
				bind:checked={reviewedPolicy}
				disabled={savingPolicy}
			/><Label for="aggregate-policy-reviewed" class="leading-relaxed"
				>ตรวจเกณฑ์นี้แล้วและยืนยันให้ใช้เป็นนโยบายของโรงเรียน</Label
			>
		</div>
		{#if policyError}<p role="alert" class="text-sm text-destructive">{policyError}</p>{/if}
		<Dialog.Footer
			><Button variant="outline" disabled={savingPolicy} onclick={() => (policyOpen = false)}
				>ยกเลิก</Button
			><LoadingButton
				loading={savingPolicy}
				disabled={!capabilities.policy || !reviewedPolicy || !policyName.trim()}
				onclick={() => void savePolicy()}>บันทึกนโยบายที่ตรวจแล้ว</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content></Dialog.Root
>
