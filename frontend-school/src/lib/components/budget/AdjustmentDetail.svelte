<script lang="ts">
	import ReadAccessNotice from './ReadAccessNotice.svelte';
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		getBudgetAdjustment,
		getBudgetProcess,
		getBudgetDashboard,
		getBudgetCatalogs,
		submitBudgetAdjustment,
		actOnBudgetAdjustment,
		type BudgetAdjustmentDetail,
		type BudgetProcessDetail,
		type BudgetDashboard,
		type BudgetCatalogs
	} from '#lib/api/budget.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { adjustmentLabels } from '#lib/budget/presentation.js';
	import { money } from '#lib/budget/money.js';
	import { message } from '#lib/budget/drafts.js';
	import { acceptedAllocation } from '#lib/budget/adjustment-context.js';
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
		fromSource,
		toSource,
		catalogsSource,
		returnTo
	}: {
		id: string;
		source: Promise<RouteLoadResult<BudgetAdjustmentDetail | null>>;
		processSource: Promise<RouteLoadResult<BudgetProcessDetail | null>>;
		fromSource: Promise<RouteLoadResult<BudgetDashboard | null>>;
		toSource: Promise<RouteLoadResult<BudgetDashboard | null>>;
		catalogsSource: Promise<RouteLoadResult<BudgetCatalogs | null>>;
		returnTo: string;
	} = $props();
	const region = new BudgetRegion<BudgetAdjustmentDetail | null>(),
		fromBudget = new BudgetRegion<BudgetDashboard | null>(),
		toBudget = new BudgetRegion<BudgetDashboard | null>(),
		catalogs = new BudgetRegion<BudgetCatalogs | null>(),
		intent = new BudgetMutationIntent();
	const lookupController = new AbortController();
	const fiscalYear = $derived(
		catalogs.value?.fiscalYears.find((year) => year.id === region.value?.fiscalYearId)
	);
	const fromAllocation = $derived(
		region.value ? acceptedAllocation(region.value.from, fromBudget.value) : null
	);
	const toAllocation = $derived(
		region.value ? acceptedAllocation(region.value.to, toBudget.value) : null
	);
	const allocationReady = $derived(
		!!region.value &&
			!!fiscalYear &&
			!catalogs.error &&
			(region.value.kind === 'add' || (!!fromAllocation && !fromBudget.error)) &&
			(region.value.kind === 'reduce' || (!!toAllocation && !toBudget.error))
	);
	let pending = $state(false),
		error = $state(''),
		editing = $state(false),
		repaying = $state(false),
		nextProcess = $state<Promise<RouteLoadResult<BudgetProcessDetail | null>> | null>(null),
		disposed = false;
	$effect.pre(() => {
		region.consume(source);
		fromBudget.consume(fromSource);
		toBudget.consume(toSource);
		catalogs.consume(catalogsSource);
	});
	onDestroy(() => {
		disposed = true;
		region.dispose();
		fromBudget.dispose();
		toBudget.dispose();
		catalogs.dispose();
		lookupController.abort();
	});
	function patch(value: BudgetAdjustmentDetail) {
		const previous = region.value;
		region.patch(value);
		if (
			previous?.from?.projectId === value.from?.projectId &&
			previous?.to?.projectId === value.to?.projectId
		)
			return;
		const reads: Partial<Record<string, Promise<BudgetDashboard>>> = {};
		for (const [ref, target] of [
			[value.from, fromBudget],
			[value.to, toBudget]
		] as const) {
			target.patch(null);
			if (!ref) continue;
			let read = reads[ref.projectId];
			if (!read) {
				read = getBudgetDashboard({
					signal: lookupController.signal,
					query: { projectId: ref.projectId }
				});
				reads[ref.projectId] = read;
			}
			target.consume(captureRouteLoad(read, 'โหลดรายละเอียดงบประมาณไม่สำเร็จ'));
		}
	}
	async function submit() {
		const value = region.value;
		if (!value || pending) return;
		pending = true;
		error = '';
		try {
			const basis = { expectedRevision: value.revision };
			const result = await submitBudgetAdjustment(id, { ...basis, requestId: intent.for(basis) });
			if (!disposed) {
				patch(result);
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
				<section aria-label="ปีงบประมาณของรายการ" class="space-y-2" aria-busy={catalogs.loading}>
					{#if catalogs.error}<PageState
							variant="error"
							title="โหลดปีงบประมาณและหมวดรายการไม่สำเร็จ"
							description={catalogs.error}
							actionLabel="ลองอีกครั้ง"
							onaction={() => catalogs.retry((signal) => getBudgetCatalogs({ signal }))}
						/>{/if}
					{#if catalogs.loading && !catalogs.value}<PageSkeleton
							variant="form"
							rows={1}
						/>{:else if fiscalYear}<p class="text-sm">
							ปีงบประมาณ <strong>{fiscalYear.name}</strong>
						</p>{:else if !catalogs.error}<PageState
							variant="permission"
							title="ไม่พบปีงบประมาณของรายการ"
							description="ติดต่อผู้ดูแลเพื่อตรวจสอบปีงบประมาณก่อนพิจารณารายการ"
						/>{/if}
				</section>
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
	{#if region.value}<div class="grid gap-3 lg:grid-cols-2">
			{#each [{ side: 'from' as const, title: 'งบประมาณต้นทาง (หักออก)', ref: region.value.from, project: fromBudget, allocation: fromAllocation }, { side: 'to' as const, title: 'งบประมาณปลายทาง (รับเข้า)', ref: region.value.to, project: toBudget, allocation: toAllocation }] as context (context.side)}{#if context.ref}<section
						class="space-y-3 rounded-xl border bg-card p-3 sm:p-4"
						aria-label={context.title}
						aria-busy={context.project.loading}
					>
						<h2 class="font-semibold">{context.title}</h2>
						{#if context.project.error}<PageState
								variant="error"
								title="โหลดรายละเอียดงบประมาณไม่สำเร็จ"
								description={context.project.error}
								actionLabel="ลองอีกครั้ง"
								onaction={() => {
									const ref = context.ref;
									if (ref)
										context.project.retry((signal) =>
											getBudgetDashboard({ signal, query: { projectId: ref.projectId } })
										);
								}}
							/>{/if}{#if context.project.loading && !context.project.value}<PageSkeleton
								variant="form"
								rows={4}
							/>{:else if context.allocation}<dl class="grid gap-3 sm:grid-cols-2">
								<div>
									<dt class="text-sm text-muted-foreground">โครงการ</dt>
									<dd class="font-medium">{context.allocation.projectName}</dd>
								</div>
								<div>
									<dt class="text-sm text-muted-foreground">กิจกรรม</dt>
									<dd>{context.allocation.activityTitle}</dd>
								</div>
								<div>
									<dt class="text-sm text-muted-foreground">แหล่งเงิน</dt>
									<dd>{context.allocation.fundName}</dd>
								</div>
								<div>
									<dt class="text-sm text-muted-foreground">หมวดรายจ่าย</dt>
									<dd>{context.allocation.categoryName}</dd>
								</div>
							</dl>{:else if !context.project.error && !catalogs.loading && !catalogs.error}<PageState
								variant="permission"
								title="ยังยืนยันรายการงบประมาณไม่ได้"
								description="ตรวจสอบสิทธิ์และรายการในแผนที่อนุมัติ หรือขอให้ผู้รับผิดชอบแก้ไขรายการก่อนพิจารณา"
							/>{/if}
					</section>{/if}{/each}
		</div>
		{#if !allocationReady && region.value.capabilities.canAct}<PageState
				variant="permission"
				title="ตรวจสอบรายละเอียดงบประมาณก่อนพิจารณา"
				description="ต้องแสดงปีงบประมาณ กิจกรรม แหล่งเงิน และหมวดรายจ่ายของรายการให้ครบก่อนยืนยันการพิจารณา"
			/>{/if}{/if}
	{#if region.value}<WorkflowPanel
			source={nextProcess ?? processSource}
			kind="adjustments"
			{id}
			revision={region.value.revision}
			canAct={allocationReady &&
				region.value.capabilities.canAct &&
				$can.has(PERMISSIONS.BUDGET_APPROVE_ASSIGNED)}
			onaction={async (input) => {
				if (!allocationReady)
					throw new Error('ตรวจสอบรายละเอียดงบประมาณต้นทางและปลายทางให้ครบก่อนพิจารณา');
				const value = await actOnBudgetAdjustment(id, input);
				if (!disposed) patch(value);
			}}
		/>{/if}{#if region.value}<History
			kind="adjustments"
			{id}
			revision={region.value.revision}
		/>{/if}</PageShell
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
					else patch(value);
					editing = false;
					repaying = false;
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
