<script lang="ts">
	import ReadAccessNotice from './ReadAccessNotice.svelte';
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		getBudgetAdjustment,
		getBudgetProcess,
		submitBudgetAdjustment,
		actOnBudgetAdjustment,
		type BudgetAdjustmentDetail,
		type BudgetProcessDetail
	} from '#lib/api/budget.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { adjustmentLabels } from '#lib/budget/presentation.js';
	import { money } from '#lib/budget/money.js';
	import { message } from '#lib/budget/drafts.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import Navigation from './Navigation.svelte';
	import WorkflowPanel from './WorkflowPanel.svelte';
	import History from './History.svelte';
	import StatusBadge from './StatusBadge.svelte';
	import AdjustmentForm from './AdjustmentForm.svelte';
	let {
		id,
		source,
		processSource,
		returnTo
	}: {
		id: string;
		source: Promise<RouteLoadResult<BudgetAdjustmentDetail | null>>;
		processSource: Promise<RouteLoadResult<BudgetProcessDetail | null>>;
		returnTo: string;
	} = $props();
	const region = new BudgetRegion<BudgetAdjustmentDetail | null>(),
		intent = new BudgetMutationIntent();
	let pending = $state(false),
		error = $state(''),
		editing = $state(false),
		repaying = $state(false),
		nextProcess = $state<Promise<RouteLoadResult<BudgetProcessDetail | null>> | null>(null),
		disposed = false;
	$effect.pre(() => region.consume(source));
	onDestroy(() => {
		disposed = true;
		region.dispose();
	});
	async function submit() {
		const value = region.value;
		if (!value || pending) return;
		pending = true;
		error = '';
		try {
			const basis = { expectedRevision: value.revision };
			const result = await submitBudgetAdjustment(id, { ...basis, requestId: intent.for(basis) });
			if (!disposed) {
				region.patch(result);
				nextProcess = captureRouteLoad(getBudgetProcess('adjustments', id), 'โหลดขั้นตอนไม่สำเร็จ');
			}
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<PageShell
	title={region.value ? adjustmentLabels[region.value.kind] : 'รายละเอียดการปรับงบประมาณ'}
	backHref={returnTo}
	backLabel="กลับรายการปรับงบ"
	contentClass="space-y-4"
	>{#snippet actions()}{#if region.value}{#if region.value.capabilities.canEdit}<Button
					variant="outline"
					onclick={() => (editing = true)}>แก้ไขฉบับร่าง</Button
				>{/if}{#if region.value.capabilities.canSubmit}<LoadingButton
					loading={pending}
					onclick={submit}>ส่งรายการพิจารณา</LoadingButton
				>{/if}{#if region.value.kind === 'loan' && region.value.status === 'approved' && BigInt(region.value.loanOutstandingSatang) > 0n && $can.has(PERMISSIONS.BUDGET_ADJUSTMENT_CREATE_SCHOOL)}<Button
					onclick={() => (repaying = true)}>คืนเงินยืม</Button
				>{/if}{/if}{/snippet}<Navigation /><ReadAccessNotice />
	<section
		aria-label="รายละเอียดการปรับงบ"
		aria-busy={region.loading}
		data-testid="budget-adjustment-detail"
	>
		{#if region.error}<PageState
				variant="error"
				title="โหลดรายการไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => getBudgetAdjustment(id, { signal }))}
			/>{/if}{#if region.loading && !region.value}<PageSkeleton
				variant="form"
				rows={4}
			/>{:else if region.value}{@const detail = region.value}
			<div
				class="space-y-3 rounded-xl border border-primary/15 bg-gradient-to-r from-primary/5 to-card p-4"
			>
				<StatusBadge status={detail.status} />
				<p class="text-xl font-semibold tabular-nums">{money(detail.amountSatang)} บาท</p>
				<p>{detail.reason}</p>
				{#if detail.kind === 'loan'}<p class="text-sm">
						เงินยืมค้างคืน <strong>{money(detail.loanOutstandingSatang)} บาท</strong>
					</p>{/if}
				<div class="flex flex-wrap gap-2">
					{#if detail.from}<Button
							variant="outline"
							size="sm"
							href={`/staff/budget/projects/${detail.from.projectId}`}>เปิดโครงการต้นทาง</Button
						>{/if}{#if detail.to}<Button
							variant="outline"
							size="sm"
							href={`/staff/budget/projects/${detail.to.projectId}`}>เปิดโครงการปลายทาง</Button
						>{/if}{#if detail.loanId}<Button
							variant="outline"
							size="sm"
							href={`/staff/budget/adjustments/${detail.loanId}`}>เปิดรายการยืม</Button
						>{/if}
				</div>
			</div>{/if}{#if error}<PageState
				variant="error"
				title="ดำเนินการไม่สำเร็จ"
				description={error}
			/>{/if}
	</section>
	{#if region.value}<WorkflowPanel
			source={nextProcess ?? processSource}
			kind="adjustments"
			{id}
			revision={region.value.revision}
			canAct={region.value.capabilities.canAct && $can.has(PERMISSIONS.BUDGET_APPROVE_ASSIGNED)}
			onaction={async (input) => {
				const value = await actOnBudgetAdjustment(id, input);
				if (!disposed) region.patch(value);
			}}
		/>{/if}{#if region.value}<History kind="adjustments" {id} />{/if}</PageShell
>
<Dialog.Root
	open={editing || repaying}
	onOpenChange={(open) => {
		if (!open) {
			editing = false;
			repaying = false;
		}
	}}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-2xl"
		><Dialog.Header
			><Dialog.Title>{repaying ? 'คืนเงินยืม' : 'แก้ไขรายการปรับงบ'}</Dialog.Title
			><Dialog.Description>ตรวจสอบงบประมาณและเหตุผลก่อนบันทึก</Dialog.Description></Dialog.Header
		>{#if (editing || repaying) && region.value}<AdjustmentForm
				detail={editing ? region.value : undefined}
				loan={repaying ? region.value : undefined}
				oncancel={() => {
					editing = false;
					repaying = false;
				}}
				onsaved={(value) => {
					if (repaying) void goto(`/staff/budget/adjustments/${value.id}`);
					else region.patch(value);
					editing = false;
					repaying = false;
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
