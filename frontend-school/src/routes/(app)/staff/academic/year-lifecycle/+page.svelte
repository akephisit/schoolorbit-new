<script lang="ts">
	import { untrack } from 'svelte';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { resolve } from '$app/paths';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import { authStore } from '#lib/stores/auth.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import {
		getYearLifecycleWorkspace,
		type YearLifecycleWorkspace,
		type TermTransitionOutcome
	} from '#lib/api/academic-lifecycle.js';
	import { listPromotionRuns, type PromotionRunList } from '#lib/api/academic-promotion.js';
	import type { YearRolloverOutcome } from '#lib/api/academic-year-management.js';
	import { academicStatusLabel } from '#lib/academic/lifecycle/year-management.js';
	import AcademicSetupRegion from '#lib/components/academic-core/AcademicSetupRegion.svelte';
	import YearRolloverDialog from '#lib/components/academic/lifecycle/YearRolloverDialog.svelte';
	import PeriodActionDialog from '#lib/components/academic/lifecycle/PeriodActionDialog.svelte';
	import AcademicRetentionSettings from '#lib/components/academic/lifecycle/AcademicRetentionSettings.svelte';
	import YearReopeningDialog from '#lib/components/academic/lifecycle/YearReopeningDialog.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	const academicContext = getAcademicContextStore();
	let workspace = $state.raw<YearLifecycleWorkspace | null>(null);
	let loading = $state(false);
	let error = $state('');
	let regionKey = '';
	let chosenTerm = $state('');
	let history = $state.raw<PromotionRunList | null>(null);
	let historyError = $state('');
	let historyLoading = $state(false);
	let historyKey = '';
	let prepared = $state<YearRolloverOutcome | null>(null);
	let preparedUnassigned = $state(0);
	let preparedJoining = $state(0);
	let activatedContext = $state<TermTransitionOutcome['context'] | null>(null);
	let activationSettingsError = $state('');
	let setupRegion = $state<{ refreshAfterLifecycle: () => Promise<void> } | null>(null);
	const canRead = $derived($can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL));
	const canReadPromotion = $derived($can.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL));
	const canSetup = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_YEAR_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_TERM_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL
		)
	);
	const canManageSettings = $derived(
		$can.hasAll(PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL, PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL)
	);
	const sourceName = $derived(
		workspace?.context.name ??
			$academicContext.options?.years.find((year) => year.id === data.academicYearId)?.name ??
			'ปีที่เลือก'
	);
	const plannedTerms = $derived(
		workspace && ['planning', 'ready', 'active'].includes(workspace.context.status)
			? workspace.terms.filter((term) => term.status === 'planning' || term.status === 'ready')
			: []
	);
	const targetTerm = $derived(
		plannedTerms.find((term) => term.academicTermId === chosenTerm) ?? plannedTerms[0]
	);
	const yearRead = new LatestRequest();
	const historyRead = new LatestRequest();
	async function reloadYear() {
		if (!canRead || !data.academicYearId) return;
		const yearId = data.academicYearId;
		const identity = appIdentityKey();
		const { revision, signal } = yearRead.begin();
		loading = true;
		error = '';
		try {
			const result = await getYearLifecycleWorkspace(yearId, { signal });
			if (yearRead.isCurrent(revision) && identity === appIdentityKey()) workspace = result;
		} catch (cause) {
			if (yearRead.isCurrent(revision) && !isAbortError(cause))
				error = cause instanceof Error ? cause.message : 'โหลดปีการศึกษาไม่สำเร็จ';
		} finally {
			if (yearRead.isCurrent(revision)) loading = false;
		}
	}
	async function reloadHistory(loadMore = false) {
		if (!canReadPromotion || !data.academicYearId) return;
		const yearId = data.academicYearId;
		const identity = appIdentityKey();
		const { revision, signal } = historyRead.begin();
		historyLoading = true;
		historyError = '';
		try {
			const cursor = loadMore ? history?.nextCursor : null;
			const previous = history;
			const result = await listPromotionRuns(
				{ sourceYearId: yearId, ...(cursor ? { beforeId: cursor } : {}) },
				{ signal }
			);
			if (historyRead.isCurrent(revision) && identity === appIdentityKey())
				history =
					cursor && previous ? { ...result, runs: [...previous.runs, ...result.runs] } : result;
		} catch (cause) {
			if (historyRead.isCurrent(revision) && !isAbortError(cause))
				historyError = cause instanceof Error ? cause.message : 'โหลดประวัติไม่สำเร็จ';
		} finally {
			if (historyRead.isCurrent(revision)) historyLoading = false;
		}
	}
	async function changedPeriod() {
		await Promise.all([reloadYear(), academicContext.retry()]);
	}
	async function handleActivated(outcome: TermTransitionOutcome) {
		activatedContext = outcome.context;
		activationSettingsError = '';
		if (!canSetup || !setupRegion) return;
		try {
			await setupRegion.refreshAfterLifecycle();
		} catch {
			activationSettingsError =
				'เริ่มใช้รอบใหม่แล้ว แต่โหลดตัวเลือกและค่าเริ่มต้นล่าสุดไม่สำเร็จ กรุณาลองโหลดในส่วนตั้งค่า';
		}
	}
	function handlePrepared(outcome: YearRolloverOutcome, unassigned: number, joining: number) {
		prepared = outcome;
		preparedUnassigned = unassigned;
		preparedJoining = joining;
		void reloadHistory();
	}

	$effect.pre(() => {
		const promise = data.workspace;
		const { revision } = yearRead.begin();
		const identityState = $authStore;
		const key = `${data.academicYearId ?? ''}:${appIdentityKey()}`;
		const allowed = canRead && identityState.isAuthenticated;
		let current = true;
		untrack(() => {
			if (key !== regionKey || !allowed) {
				workspace = null;
				chosenTerm = '';
				prepared = null;
				activatedContext = null;
				activationSettingsError = '';
			}
			regionKey = key;
			loading = !!promise && allowed;
			error = '';
		});
		if (promise && allowed)
			void promise.then((result) => {
				if (!current || !yearRead.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) workspace = result.data;
					else error = result.error;
					loading = false;
				});
			});
		return () => {
			current = false;
			yearRead.abort();
		};
	});
	$effect.pre(() => {
		const promise = data.history;
		const { revision } = historyRead.begin();
		const identityState = $authStore;
		const key = `${data.academicYearId ?? ''}:${appIdentityKey()}`;
		const allowed = canReadPromotion && identityState.isAuthenticated;
		let current = true;
		untrack(() => {
			if (key !== historyKey || !allowed) history = null;
			historyKey = key;
			historyLoading = !!promise && allowed;
			historyError = '';
		});
		if (promise && allowed)
			void promise.then((result) => {
				if (!current || !historyRead.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) history = result.data;
					else historyError = result.error;
					historyLoading = false;
				});
			});
		return () => {
			current = false;
			historyRead.abort();
		};
	});
</script>

<PageShell
	title="จัดการปีการศึกษา"
	description="ตั้งค่าปีและภาคเรียน เปลี่ยนภาคเรียน และเตรียมรายชื่อนักเรียนปีใหม่ในที่เดียว"
>
	{#if !data.academicYearId}<PageState
			variant="empty"
			title="เลือกปีการศึกษา"
			description="เลือกปีที่ต้องการจัดการจากแถบด้านบน หรือเพิ่มปีการศึกษาในส่วนตั้งค่าด้านล่าง"
		/>{:else}
		<section
			class="space-y-4 rounded-xl border bg-card p-4 sm:p-5"
			aria-label="จัดการรอบการศึกษา"
			aria-busy={loading}
		>
			<div class="flex flex-wrap items-start justify-between gap-4">
				<div>
					<h2 class="text-xl font-semibold">{sourceName}</h2>
					{#if workspace}<Badge variant="outline"
							>{academicStatusLabel(workspace.context.status)}</Badge
						>{/if}
					<p class="mt-2 text-sm text-muted-foreground">
						เปลี่ยนภาคเรียนใช้ชั้นและห้องเดิม การขึ้นปีใหม่เตรียมรายชื่อก่อน และเริ่มใช้ปีใหม่แยกกัน
					</p>
				</div>
				{#key data.academicYearId}<YearRolloverDialog
						yearId={data.academicYearId}
						yearName={sourceName}
						onprepared={handlePrepared}
					/>{/key}
			</div>
			{#if activatedContext}<section
					class="space-y-3 rounded-xl border border-primary/30 bg-primary/5 p-4"
					aria-label="ตั้งค่าหลังเริ่มใช้รอบใหม่"
					role="status"
				>
					<h3 class="font-semibold">
						เริ่มใช้ {activatedContext.yearName} · {activatedContext.termName} แล้ว
					</h3>
					<p class="text-sm">
						หากต้องการให้ครูและนักเรียนเปิดมาที่รอบนี้ ให้แสดงปีและภาคเรียนในตัวเลือก
						แล้วตั้งภาคเรียนเป็นค่าเริ่มต้นของโรงเรียน
					</p>
					{#if canSetup}<p class="text-sm">
							ในส่วนตั้งค่าด้านล่าง เลือก {activatedContext.yearName} ด้วยช่อง “ปีที่ต้องการตั้งค่า” แล้วใช้
							“ตัวเลือกและค่าเริ่มต้นของโรงเรียน” ลิงก์นี้พาไปยังส่วนตั้งค่า โดยคงปีและร่างที่กำลังแก้ไขไว้
						</p>
						<Button variant="outline" href="#academic-year-settings"
							>ไปที่ตัวเลือกและค่าเริ่มต้น</Button
						>{/if}
					{#if !canManageSettings}<p class="text-sm">
							ให้ผู้ดูแลที่มีสิทธิ์จัดการปีและภาคเรียนดำเนินการตั้งค่าตัวเลือกและค่าเริ่มต้นนี้
						</p>{/if}
					{#if activationSettingsError}<p role="alert" class="text-sm text-destructive">
							{activationSettingsError}
						</p>{/if}
				</section>{/if}
			{#if prepared}<div
					class="space-y-2 rounded-xl border border-primary/30 bg-primary/5 p-4"
					role="status"
				>
					<p class="font-medium">
						บันทึกการขึ้นปีใหม่แล้ว {prepared.studentCount} คน · ยังไม่เริ่มใช้
					</p>
					<p class="text-sm">
						เตรียมนักเรียนเข้าเรียนปีใหม่ {preparedJoining} คน ปีเดิมและประวัติห้องเรียนยังอยู่ เลือกเริ่มใช้เมื่อโรงเรียนพร้อม
					</p>
					{#if preparedUnassigned > 0}<p class="text-sm">
							ยังไม่จัดห้อง {preparedUnassigned} คน ยังไม่พร้อมใช้เช็กชื่อในห้องเรียน
						</p>
						{#if $can.has(PERMISSIONS.HOMEROOM_MANAGE_SCHOOL)}<Button
								variant="outline"
								href={resolve(`staff/academic/homerooms?academicYearId=${prepared.targetYearId}`)}
								>จัดห้องปีปลายทาง</Button
							>{/if}{/if}<Button
						href={resolve(`staff/academic/year-lifecycle?academicYearId=${prepared.targetYearId}`)}
						>ไปเริ่มใช้ปีใหม่</Button
					><Button variant="outline" href={resolve(`staff/academic/promotion/${prepared.runId}`)}
						>ดูหลักฐานการเตรียมรายชื่อ</Button
					>
				</div>{/if}
			{#if loading && !workspace}<PageSkeleton
					variant="table"
					rows={3}
					columns={3}
				/>{:else if error}<PageState
					variant="error"
					title="โหลดปีการศึกษาไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={reloadYear}
				/>{/if}
			{#if workspace}
				<div class="overflow-x-auto">
					<Table.Root
						><Table.Header
							><Table.Row
								><Table.Head>ภาคเรียน</Table.Head><Table.Head>สถานะ</Table.Head><Table.Head
									>ดำเนินการ</Table.Head
								></Table.Row
							></Table.Header
						><Table.Body
							>{#each workspace.terms as term (term.academicTermId)}<Table.Row
									><Table.Cell>{term.name}</Table.Cell><Table.Cell
										>{academicStatusLabel(term.status)}</Table.Cell
									><Table.Cell
										>{#if term.status === 'active' || term.status === 'closing'}<PeriodActionDialog
												yearId={workspace.context.academicYearId}
												termId={term.academicTermId}
												kind="close_term"
												label="ปิดภาคเรียน"
												disabled={loading}
												oncompleted={changedPeriod}
											/>{/if}</Table.Cell
									></Table.Row
								>{/each}</Table.Body
						></Table.Root
					>
				</div>
				<div class="flex flex-wrap items-end gap-3">
					{#if plannedTerms.length > 0}<div class="w-full max-w-sm space-y-2">
							<Label for="next-term">ภาคเรียนที่จะเริ่มใช้</Label><Select.Root
								type="single"
								value={targetTerm?.academicTermId ?? ''}
								onValueChange={(value) => {
									chosenTerm = value;
								}}
								><Select.Trigger id="next-term" class="w-full"
									><span>{targetTerm?.name ?? 'เลือกภาคเรียน'}</span></Select.Trigger
								><Select.Content
									>{#each plannedTerms as term (term.academicTermId)}<Select.Item
											value={term.academicTermId}>{term.name}</Select.Item
										>{/each}</Select.Content
								></Select.Root
							>
						</div>
						{#if targetTerm}{#key targetTerm.academicTermId}<PeriodActionDialog
									yearId={workspace.context.academicYearId}
									termId={targetTerm.academicTermId}
									kind="activate"
									label={workspace.context.status === 'planning' ||
									workspace.context.status === 'ready'
										? 'เริ่มใช้ปีใหม่'
										: 'เปลี่ยนภาคเรียน'}
									disabled={loading}
									oncompleted={changedPeriod}
									onactivated={handleActivated}
								/>{/key}{/if}{/if}{#if workspace.context.status === 'active' || workspace.context.status === 'closing'}<PeriodActionDialog
							yearId={workspace.context.academicYearId}
							kind="close_year"
							label="ปิดปีการศึกษา"
							disabled={loading}
							oncompleted={changedPeriod}
						/>{:else if workspace.context.status === 'closed'}<YearReopeningDialog
							yearId={workspace.context.academicYearId}
							yearName={workspace.context.name}
							onreopened={changedPeriod}
						/>{/if}
				</div>
			{/if}
		</section>
		{#if canRead}{#key data.academicYearId}<AcademicRetentionSettings
					yearId={data.academicYearId}
					terms={workspace?.terms ?? []}
				/>{/key}{/if}
		{#if canReadPromotion}<section
				class="space-y-3 rounded-xl border bg-card p-4 sm:p-5"
				aria-label="ประวัติขึ้นปีการศึกษาใหม่"
				aria-busy={historyLoading}
			>
				<h2 class="text-lg font-semibold">ประวัติขึ้นปีการศึกษาใหม่</h2>
				{#if historyLoading && !history}<PageSkeleton
						variant="table"
						rows={2}
						columns={2}
					/>{/if}{#if historyError}<PageState
						variant="error"
						title="โหลดประวัติไม่สำเร็จ"
						description={historyError}
						actionLabel="ลองอีกครั้ง"
						onaction={reloadHistory}
					/>{/if}{#if history}{#each history.runs as run (run.id)}<div
							class="flex flex-wrap items-center justify-between gap-2 rounded-lg border p-3"
						>
							<p>
								บันทึก {new Date(run.createdAt).toLocaleDateString('th-TH', {
									timeZone: 'Asia/Bangkok'
								})}
							</p>
							<Button variant="outline" href={resolve(`staff/academic/promotion/${run.id}`)}
								>ดูประวัติ</Button
							>
						</div>{:else}<p class="text-sm text-muted-foreground">
							ยังไม่มีประวัติขึ้นปีใหม่จากปีนี้
						</p>{/each}{/if}

				{#if history?.nextCursor}<Button
						variant="outline"
						disabled={historyLoading}
						onclick={() => void reloadHistory(true)}>ดูประวัติเก่ากว่านี้</Button
					>{/if}
			</section>{/if}
	{/if}
	{#if canSetup}<div id="academic-year-settings">
			<AcademicSetupRegion bind:this={setupRegion} workspaceLoad={data.setup} />
		</div>{/if}
</PageShell>
