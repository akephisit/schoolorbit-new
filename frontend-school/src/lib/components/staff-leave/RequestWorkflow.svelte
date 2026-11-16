<script lang="ts">
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		getLeaveRequest,
		submitLeaveRequest,
		type LeaveKind,
		type LeavePersonalOptions,
		type LeaveRequestDetail
	} from '#lib/api/staff-leave.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import { leaveHome, leaveDate, leaveAmount } from '#lib/staff-leave/presentation.js';
	import { safeLeaveReturn } from '#lib/staff-leave/filters.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import RequestEditor from './RequestEditor.svelte';
	let {
		source,
		optionsSource,
		kind = 'leave',
		id
	}: {
		source?: Promise<RouteLoadResult<LeaveRequestDetail | null>>;
		optionsSource: Promise<RouteLoadResult<LeavePersonalOptions | null>>;
		kind?: LeaveKind;
		id?: string;
	} = $props();
	const region = new LeaveRegion<LeaveRequestDetail | null>();
	const submitIntent = new LeaveMutationIntent();
	const detail = $derived(region.value);
	const effectiveKind = $derived(detail?.request.kind ?? kind);
	const effectiveUrl = $derived(page.shallow?.url ?? page.url);
	const step = $derived(effectiveUrl.searchParams.get('step') ?? 'details');
	const backHref = $derived(
		safeLeaveReturn(effectiveUrl.searchParams.get('returnTo'), leaveHome(effectiveKind))
	);
	const creator = $derived($can.has(PERMISSIONS.STAFF_LEAVE_CREATE_OWN));
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
	function saved(result: LeaveRequestDetail, proceed: boolean) {
		region.patch(result);
		if (!id) {
			void goto(
				`/staff/leave/requests/${result.request.id}/edit?returnTo=${encodeURIComponent(backHref)}&step=${proceed ? 'coverage' : 'details'}`
			);
		} else if (proceed) selectStep('coverage');
	}
	async function submit() {
		if (!detail || !creator || pending) return;
		pending = true;
		error = '';
		const basis = { expectedRevision: detail.request.revision };
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
	title={id
		? 'จัดเตรียมคำขอ'
		: effectiveKind === 'exit'
			? 'สร้างคำขอออกนอกโรงเรียน'
			: 'สร้างคำขอการลา'}
	description="รายละเอียดคำขอ · จัดคาบสอน · ตรวจสอบและส่ง"
	{backHref}
	backLabel="กลับรายการคำขอ"
	contentClass="space-y-5"
>
	<nav
		aria-label="ขั้นตอนเตรียมคำขอ"
		class="grid grid-cols-3 gap-2 rounded-xl border border-primary/20 bg-primary/5 p-3"
	>
		{#each [{ id: 'details', label: 'รายละเอียด' }, { id: 'coverage', label: 'จัดคาบสอน' }, { id: 'review', label: 'ตรวจสอบและส่ง' }] as item, index (item.id)}
			<Button
				variant={step === item.id ? 'default' : 'ghost'}
				disabled={item.id !== 'details' && !detail}
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
				oncancel={() => goto(backHref)}
				onsaved={saved}
				onrefresh={refreshed}
			/>
		</div>
		{#if step === 'coverage' && detail}<section
				class="space-y-4 rounded-xl border bg-card p-4 sm:p-6"
				data-testid="leave-coverage-step"
			>
				<h2 class="text-lg font-semibold">คาบสอนที่ได้รับผลกระทบ</h2>
				<p class="text-sm text-muted-foreground">
					ช่วงวันที่ {leaveDate(detail.request.startDate)} – {leaveDate(detail.request.endDate)}
				</p>
				<div class="flex justify-between gap-2">
					<Button variant="outline" onclick={() => selectStep('details')}>กลับรายละเอียด</Button
					><Button onclick={() => selectStep('review')}>ตรวจสอบคำขอ</Button>
				</div>
			</section>{/if}
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
						<dt class="text-sm text-muted-foreground">ช่วงวันที่</dt>
						<dd>{leaveDate(detail.request.startDate)} – {leaveDate(detail.request.endDate)}</dd>
					</div>
					<div>
						<dt class="text-sm text-muted-foreground">หลักฐาน</dt>
						<dd>{detail.evidenceFileIds.length} รายการ</dd>
					</div>
					<div class="sm:col-span-2">
						<dt class="text-sm text-muted-foreground">เหตุผล</dt>
						<dd class="whitespace-pre-wrap break-words">{detail.request.reason}</dd>
					</div>
				</dl>
				{#if error}<PageState
						variant="error"
						title="ส่งคำขอไม่สำเร็จ"
						description={error}
						actionLabel="ตรวจสอบข้อมูลล่าสุด"
						onaction={refreshed}
					/>{/if}
				<div class="flex justify-between gap-2">
					<Button variant="outline" disabled={pending} onclick={() => selectStep('coverage')}
						>กลับจัดคาบสอน</Button
					><LoadingButton loading={pending} disabled={!detail.canSubmit} onclick={submit}
						>ส่งคำขอพิจารณา</LoadingButton
					>
				</div>
			</section>{/if}
	{/if}
</PageShell>
