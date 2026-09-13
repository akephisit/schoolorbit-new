<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { toast } from 'svelte-sonner';
	import { RefreshCw, Settings2 } from 'lucide-svelte';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
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
	import { PageState, PageSkeleton, LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import { can } from '$lib/stores/permissions';

	const context = getAcademicContextStore();
	const listRequest = new LatestRequest();
	const detailRequest = new LatestRequest();
	let students = $state.raw<AggregateStudent[]>([]);
	let policies = $state.raw<AggregatePolicyVersion[]>([]);
	let preview = $state.raw<TermAggregatePreview | null>(null);
	let history = $state.raw<TermAggregateRevision[]>([]);
	let selected = $state('');
	let policyId = $state('');
	let search = $state('');
	let visibleCount = $state(50);
	let loading = $state(false);
	let detailLoading = $state(false);
	let error = $state('');
	let detailError = $state('');
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
	let loadedContext = '';
	let pending: { key: string; body: AggregateLockInput } | null = null;
	const capabilities = $derived(aggregateCapabilities($can));
	const year = $derived($context.selected.academicYearId);
	const term = $derived($context.selected.academicTermId);
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

	async function loadRoster() {
		if (!contextQuery || !capabilities.read) return;
		const { revision, signal } = listRequest.begin();
		loading = true;
		error = '';
		try {
			const rows = await listAggregateStudents(contextQuery, { signal });
			const versions = await listAggregatePolicies({ signal });
			if (!listRequest.isCurrent(revision)) return;
			students = rows;
			policies = versions;
			if (!versions.some((version) => version.id === policyId)) policyId = versions[0]?.id ?? '';
		} catch (cause) {
			if (!isAbortError(cause) && listRequest.isCurrent(revision))
				error = cause instanceof Error ? cause.message : 'โหลดข้อมูลไม่สำเร็จ';
		} finally {
			if (listRequest.isCurrent(revision)) loading = false;
		}
	}
	async function inspect(id = selected) {
		if (!contextQuery || !capabilities.read || !id) return;
		selected = id;
		preview = null;
		history = [];
		detailError = '';
		const { revision, signal } = detailRequest.begin();
		detailLoading = true;
		try {
			const revisions = await listTermAggregateRevisions(id, contextQuery, { signal });
			const calculated = policyId
				? await previewTermAggregate(id, contextQuery, policyId, { signal })
				: null;
			if (!detailRequest.isCurrent(revision)) return;
			history = revisions;
			preview = calculated;
			stale = false;
		} catch (cause) {
			if (!isAbortError(cause) && detailRequest.isCurrent(revision))
				detailError = cause instanceof Error ? cause.message : 'ตรวจผลไม่สำเร็จ';
		} finally {
			if (detailRequest.isCurrent(revision)) detailLoading = false;
		}
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
			policyOpen = false;
			reviewedPolicy = false;
			toast.success('บันทึกนโยบายที่ตรวจแล้ว');
			if (selected) await inspect();
		} catch (cause) {
			policyError = cause instanceof Error ? cause.message : 'บันทึกนโยบายไม่สำเร็จ';
		} finally {
			savingPolicy = false;
		}
	}
	onMount(() => {
		const unregister = registerAcademicContextDirtySource(
			'term-aggregate',
			() => lockOpen || policyOpen || locking || savingPolicy
		);
		const unsubscribe = context.subscribe((state) => {
			const key =
				state.status === 'ready' && state.selected.academicYearId && state.selected.academicTermId
					? `${state.selected.academicYearId}:${state.selected.academicTermId}`
					: '';
			if (key && key !== loadedContext) {
				loadedContext = key;
				detailRequest.abort();
				students = [];
				preview = null;
				history = [];
				selected = '';
				policyId = '';
				lockOpen = false;
				policyOpen = false;
				void loadRoster().then(() => {
					const id = page.url.searchParams.get('studentAcademicYearId');
					if (id && students.some((row) => row.studentAcademicYearId === id)) void inspect(id);
				});
			} else if (!key && state.status !== 'loading') {
				loadedContext = '';
				listRequest.abort();
				detailRequest.abort();
				students = [];
				preview = null;
				history = [];
				selected = '';
				lockOpen = false;
				policyOpen = false;
			}
		});
		return () => {
			unregister();
			unsubscribe();
			listRequest.abort();
			detailRequest.abort();
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
	{:else if loading && students.length === 0}<PageSkeleton variant="table" rows={5} columns={4} />
	{:else if error}<PageState
			variant="error"
			title="โหลดผลสรุปไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => void loadRoster()}
		/>
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
						if (selected) void inspect();
					}}
					disabled={locking || detailLoading}
					><Select.Trigger id="aggregate-policy" class="w-full"
						>{activePolicy?.name ?? 'ยังไม่มีนโยบายที่ตรวจแล้ว'}</Select.Trigger
					><Select.Content
						>{#each policies as policy (policy.id)}<Select.Item value={policy.id}
								>{policy.name}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
			</div>
		</div>
		{#if policies.length === 0}<PageState
				title="ยังไม่มีนโยบายผลสรุปที่ตรวจแล้ว"
				description="ฝ่ายวิชาการต้องตรวจเกณฑ์ผ่าน ผลประเมินขั้นต่ำ และการยอมรับผลค้างก่อน จึงจะคำนวณและล็อกผลสรุปได้"
			/>{/if}
		<div class="grid items-start gap-5 xl:grid-cols-[minmax(22rem,0.9fr)_minmax(0,1.1fr)]">
			<section class="overflow-hidden rounded-xl border bg-card" aria-label="รายชื่อนักเรียน">
				<div class="border-b p-4 text-sm">
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
			</section>
			<section class="space-y-4 rounded-xl border bg-card p-4 sm:p-5" aria-label="รายละเอียดผลสรุป">
				{#if !student}<PageState
						title="เลือกนักเรียนเพื่อตรวจผล"
						description="รายการนี้รวมคนที่ยังไม่มีผลสรุปด้วย การเปิดดูยังไม่ล็อกผล"
					/>
				{:else}
					<div class="flex items-start justify-between gap-3">
						<div>
							<h2 class="font-semibold">{student.studentName}</h2>
							<p class="mt-1 text-sm text-muted-foreground">
								{student.gradeLevelName} · {student.studyProgramName}
							</p>
						</div>
						<Button
							size="sm"
							variant="outline"
							disabled={detailLoading || locking}
							onclick={() => void inspect()}>คำนวณใหม่</Button
						>
					</div>
					{#if detailLoading}<PageSkeleton variant="table" rows={3} columns={2} />
					{:else if detailError}<PageState
							variant="error"
							title="ตรวจผลไม่สำเร็จ"
							description={detailError}
						/>
					{:else if preview}
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
					{#if !detailLoading && history.length}<div class="space-y-3 border-t pt-4">
							<h3 class="text-sm font-semibold">ประวัติผลสรุปที่ล็อกแล้ว</h3>
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
