<script lang="ts">
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		getLeaveRequest,
		submitLeaveRequest,
		type LeaveCoverageOptions,
		type LeaveKind,
		type LeavePersonalOptions,
		type LeaveRequestDetail
	} from '#lib/api/staff-leave.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import {
		leaveHome,
		leaveKindLabels,
		leavePeriodCaption,
		leaveAmount
	} from '#lib/staff-leave/presentation.js';
	import { safeLeaveReturn } from '#lib/staff-leave/filters.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import CoverageEditor from './CoverageEditor.svelte';
	import { coverageCounts, coverageStateLabels, coverageReady } from '#lib/staff-leave/coverage.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import RequestEditor from './RequestEditor.svelte';
	let {
		source,
		optionsSource,
		coverageOptionsSource,
		kind = 'leave',
		id
	}: {
		source?: Promise<RouteLoadResult<LeaveRequestDetail | null>>;
		coverageOptionsSource: Promise<RouteLoadResult<LeaveCoverageOptions | null>>;
		optionsSource: Promise<RouteLoadResult<LeavePersonalOptions | null>>;
		kind?: LeaveKind;
		id?: string;
	} = $props();
	const region = new LeaveRegion<LeaveRequestDetail | null>();
	const submitIntent = new LeaveMutationIntent();
	const detail = $derived(region.value);
	const effectiveKind = $derived(detail?.request.kind ?? kind);
	const effectiveUrl = $derived(page.shallow?.url ?? page.url);
	const requestedStep = $derived(effectiveUrl.searchParams.get('step'));
	const step = $derived(
		detail && (requestedStep === 'coverage' || requestedStep === 'review')
			? requestedStep
			: 'details'
	);
	const backHref = $derived(
		safeLeaveReturn(effectiveUrl.searchParams.get('returnTo'), leaveHome(effectiveKind))
	);
	const creator = $derived($can.has(PERMISSIONS.STAFF_LEAVE_CREATE_OWN));
	let coverageRevision = $state<number | undefined>();
	let emergencyReason = $state(''),
		coverageDirty = $state(false),
		detailsDirty = $state(false);
	let pending = $state(false),
		error = $state('');
	let disposed = false;
	$effect.pre(() => {
		if (source) region.consume(source);
	});
	onDestroy(() => {
		disposed = true;
		region.dispose();
	});
	function selectStep(value: string) {
		const url = new URL(effectiveUrl.href);
		url.searchParams.set('step', value);
		void goto(`${url.pathname}${url.search}`, { shallow: true, reset: false });
	}
	function refreshed() {
		if (id) region.retry((signal) => getLeaveRequest(id, { signal }));
	}
	function coverageSaved(result: LeaveRequestDetail) {
		coverageRevision = result.request.revision;
		region.patch(result);
	}
	function saved(result: LeaveRequestDetail, proceed: boolean) {
		region.patch(result);
		if (!id) {
			void goto(
				`/staff/leave/requests/${result.request.id}/edit?returnTo=${encodeURIComponent(backHref)}&step=${proceed ? 'coverage' : 'details'}`
			);
		} else if (proceed) selectStep('coverage');
	}
	async function submit(emergency = false) {
		if (!detail || !creator || pending) return;
		pending = true;
		error = '';
		const basis = {
			expectedRevision: detail.request.revision,
			emergencyReason: emergency ? emergencyReason.trim() : null
		};
		try {
			const result = await submitLeaveRequest(detail.request.id, {
				requestId: submitIntent.for(basis),
				...basis
			});
			if (!disposed) {
				region.patch(result);
				void goto(
					`/staff/leave/requests/${result.request.id}?returnTo=${encodeURIComponent(backHref)}`
				);
			}
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ส่งคำขอไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<PageShell
	title={id ? 'จัดเตรียมคำขอ' : `สร้างคำขอ${leaveKindLabels[effectiveKind]}`}
	{backHref}
	backLabel="กลับรายการคำขอ"
	contentClass="space-y-5"
>
	{#snippet actions()}{#if detail}<Button
				variant="outline"
				href={`/staff/leave/requests/${detail.request.id}?returnTo=${encodeURIComponent(backHref)}`}
				>เปิดรายละเอียดคำขอ</Button
			>{/if}{/snippet}
	<nav
		aria-label="ขั้นตอนเตรียมคำขอ"
		class="grid grid-cols-3 gap-1 rounded-xl border border-primary/20 bg-primary/5 p-2 sm:gap-2 sm:p-3"
	>
		{#each [{ id: 'details', label: 'รายละเอียด' }, { id: 'coverage', label: 'จัดคาบสอน' }, { id: 'review', label: 'ตรวจสอบและส่ง' }] as item, index (item.id)}
			<Button
				variant={step === item.id ? 'default' : 'ghost'}
				class="h-auto min-h-10 px-1 whitespace-normal"
				disabled={(item.id !== 'details' && !detail) ||
					(item.id === 'review' && (coverageDirty || detailsDirty))}
				aria-current={step === item.id ? 'step' : undefined}
				onclick={() => selectStep(item.id)}>{index + 1}. {item.label}</Button
			>
		{/each}
	</nav>
	{#if !creator}<PageState title="ไม่มีสิทธิ์จัดเตรียมคำขอ" />
	{:else if source && region.loading && !detail}<PageSkeleton variant="form" />
	{:else if region.error && !detail}<PageState
			variant="error"
			title="โหลดคำขอไม่สำเร็จ"
			description={region.error}
			actionLabel="ลองอีกครั้ง"
			onaction={refreshed}
		/>
	{:else if detail && !detail.canEdit}<PageState title="คำขอนี้แก้ไขไม่ได้ในสถานะปัจจุบัน" /><Button
			href={`/staff/leave/requests/${detail.request.id}?returnTo=${encodeURIComponent(backHref)}`}
			>เปิดรายละเอียดคำขอ</Button
		>
	{:else}
		<div
			hidden={step !== 'details'}
			class="mx-auto w-full max-w-4xl rounded-xl border bg-card p-4 sm:p-6"
		>
			<RequestEditor
				kind={effectiveKind}
				detail={detail ?? undefined}
				{optionsSource}
				{coverageOptionsSource}
				{coverageRevision}
				ondirty={(value) => (detailsDirty = value)}
				oncancel={() => goto(backHref)}
				onsaved={saved}
				onrefresh={refreshed}
			/>
		</div>

		{#if detail}<div hidden={step !== 'coverage'} class="space-y-4">
				{#if detailsDirty}<PageState
						title="รายละเอียดมีการแก้ไขที่ยังไม่บันทึก"
						description="แผนคาบนี้ใช้รายละเอียดฉบับที่บันทึกไว้ กลับไปบันทึกวันที่และข้อมูลก่อนตรวจสอบและส่ง"
					/>{/if}
				<CoverageEditor
					{detail}
					onsaved={coverageSaved}
					onrefresh={refreshed}
					ondirty={(value) => (coverageDirty = value)}
				/>
				<div class="flex justify-between gap-2">
					<Button variant="outline" onclick={() => selectStep('details')}>กลับรายละเอียด</Button
					><Button disabled={coverageDirty || detailsDirty} onclick={() => selectStep('review')}
						>ตรวจสอบคำขอ</Button
					>
				</div>
			</div>{/if}

		{#if step === 'review' && detail}<section
				class="space-y-4 rounded-xl border bg-card p-4 sm:p-6"
				data-testid="leave-review-step"
			>
				<h2 class="text-lg font-semibold">ตรวจสอบก่อนส่ง</h2>
				<dl class="grid gap-3 sm:grid-cols-2">
					<div>
						<dt class="text-sm text-muted-foreground">ประเภทคำขอ</dt>
						<dd>{detail.request.typeName}</dd>
					</div>
					<div>
						<dt class="text-sm text-muted-foreground">จำนวน</dt>
						<dd>{leaveAmount(detail.request.amount, detail.request.unit)}</dd>
					</div>
					<div>
						<dt class="text-sm text-muted-foreground">ช่วงที่ขอ</dt>
						<dd>{leavePeriodCaption(detail.request)}</dd>
					</div>
					<div>
						<dt class="text-sm text-muted-foreground">หลักฐาน</dt>
						<dd>{detail.evidenceFileIds.length} รายการ</dd>
					</div>
					{#if detail.request.destination}
						<div>
							<dt class="text-sm text-muted-foreground">สถานที่ไปราชการ</dt>
							<dd class="break-words">{detail.request.destination}</dd>
						</div>
					{/if}
					{#if detail.request.companions?.length}
						<div class="sm:col-span-2">
							<dt class="text-sm text-muted-foreground">ผู้ร่วมเดินทาง</dt>
							<dd class="break-words">
								{detail.request.companions.map((user) => user.displayName).join(' · ')}
							</dd>
						</div>
					{/if}
					<div class="sm:col-span-2">
						<dt class="text-sm text-muted-foreground">เหตุผล</dt>
						<dd class="whitespace-pre-wrap break-words">{detail.request.reason}</dd>
					</div>
				</dl>
				{#if detail.submissionBlockingReason}<PageState
						title="คำขอนี้ยังส่งพิจารณาไม่ได้"
						description={detail.submissionBlockingReason}
					/>{/if}
				{#if detail.coverage}<div class="rounded-lg border p-3 text-sm">
						<p class="font-medium">{coverageStateLabels[detail.coverage.state]}</p>
						<p>
							เลือกครู {coverageCounts(detail.coverage.days.flatMap((day) => day.lessons))
								.selected}/{coverageCounts(detail.coverage.days.flatMap((day) => day.lessons))
								.total} · ตอบรับ {coverageCounts(detail.coverage.days.flatMap((day) => day.lessons))
								.confirmed}/{coverageCounts(detail.coverage.days.flatMap((day) => day.lessons))
								.total}
						</p>
					</div>{/if}
				<div class="space-y-2">
					<Label for="leave-emergency-reason">เหตุผลกรณีเร่งด่วนที่ยังจัดคาบไม่ครบ</Label><Textarea
						id="leave-emergency-reason"
						bind:value={emergencyReason}
						maxlength={2000}
						disabled={pending}
						rows={2}
					/>
					<p class="text-xs text-muted-foreground">
						หลังส่งคำขอ ครูที่เลือกจะตอบรับคาบสอน เมื่อส่งแบบเร่งด่วน
						ผู้ดูแลต้องจัดแผนและรอครูตอบรับให้ครบก่อนอนุมัติ
					</p>
				</div>
				{#if error}<PageState
						variant="error"
						title="ส่งคำขอไม่สำเร็จ"
						description={error}
						actionLabel="ตรวจสอบข้อมูลล่าสุด"
						onaction={refreshed}
					/>{/if}
				<div class="flex flex-wrap justify-between gap-2">
					<Button variant="outline" disabled={pending} onclick={() => selectStep('coverage')}
						>กลับจัดคาบสอน</Button
					><LoadingButton
						loading={pending}
						disabled={!detail.canSubmit || !coverageReady(detail.coverage)}
						onclick={() => submit(false)}>ส่งคำขอพิจารณา</LoadingButton
					><LoadingButton
						variant="outline"
						loading={pending}
						disabled={!detail.canSubmit || !emergencyReason.trim()}
						onclick={() => submit(true)}>ส่งคำขอเร่งด่วน</LoadingButton
					>
				</div>
			</section>{/if}
	{/if}
</PageShell>
