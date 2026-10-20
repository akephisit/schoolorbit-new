<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		getBudgetRequest,
		getBudgetProject,
		getBudgetProcess,
		listBudgetPayments,
		submitBudgetRequest,
		closeBudgetRequest,
		actOnBudgetRequest,
		type BudgetSpendingRequestDetail,
		type BudgetProjectWorkspace,
		type BudgetProcessDetail,
		type BudgetPaymentPage,
		type BudgetPayment
	} from '#lib/api/budget.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { message } from '#lib/budget/drafts.js';
	import { money } from '#lib/budget/money.js';
	import { dateLabel } from '#lib/budget/presentation.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import {
		PageState,
		PageSkeleton,
		TableSkeleton,
		LoadingButton
	} from '#lib/components/app-state/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import Navigation from './Navigation.svelte';
	import RequestForm from './RequestForm.svelte';
	import PaymentForm from './PaymentForm.svelte';
	import WorkflowPanel from './WorkflowPanel.svelte';
	import EvidenceList from './EvidenceList.svelte';
	import History from './History.svelte';
	import StatusBadge from './StatusBadge.svelte';
	let {
		id,
		source,
		processSource,
		paymentsSource,
		returnTo
	}: {
		id: string;
		source: Promise<RouteLoadResult<BudgetSpendingRequestDetail | null>>;
		processSource: Promise<RouteLoadResult<BudgetProcessDetail | null>>;
		paymentsSource: Promise<RouteLoadResult<BudgetPaymentPage | null>>;
		returnTo: string;
	} = $props();
	const region = new BudgetRegion<BudgetSpendingRequestDetail | null>(),
		payments = new BudgetRegion<BudgetPaymentPage | null>(),
		intent = new BudgetMutationIntent();
	let nextProcess = $state<Promise<RouteLoadResult<BudgetProcessDetail | null>> | null>(null),
		editing = $state(false),
		paying = $state(false),
		closing = $state(false),
		refund = $state<BudgetPayment | null>(null),
		closeReason = $state(''),
		error = $state(''),
		pending = $state(false),
		workspace = $state<BudgetProjectWorkspace | null>(null),
		editLoading = $state(false),
		editError = $state(''),
		paymentOffset = $state(0),
		disposed = false;
	const controller = new AbortController();
	const lineNames = $derived(
		Object.fromEntries(
			(region.value?.lines ?? []).map((line) => [line.planLineId, line.description])
		)
	);
	$effect.pre(() => {
		region.consume(source);
		payments.consume(paymentsSource);
	});
	onDestroy(() => {
		disposed = true;
		region.dispose();
		payments.dispose();
		controller.abort();
	});
	function loadPayments(next = paymentOffset) {
		paymentOffset = next;
		payments.retry((signal) =>
			listBudgetPayments(id, { signal, query: { limit: 20, offset: paymentOffset } })
		);
	}
	function patch(value: BudgetSpendingRequestDetail) {
		region.patch(value);
		editing = false;
		paying = false;
		refund = null;
	}
	async function edit() {
		const value = region.value;
		if (!value) return;
		editing = true;
		editLoading = true;
		editError = '';
		try {
			const result = await getBudgetProject(value.request.projectId, { signal: controller.signal });
			if (!disposed) workspace = result;
		} catch (cause) {
			if (!disposed) editError = message(cause);
		} finally {
			if (!disposed) editLoading = false;
		}
	}
	async function submit() {
		const value = region.value;
		if (!value || pending) return;
		pending = true;
		error = '';
		try {
			const basis = { expectedRevision: value.request.revision };
			const result = await submitBudgetRequest(id, {
				...basis,
				requestId: intent.for({ operation: 'submit', ...basis })
			});
			if (!disposed) {
				patch(result);
				nextProcess = captureRouteLoad(getBudgetProcess('requests', id), 'โหลดขั้นตอนไม่สำเร็จ');
			}
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
	async function close(event: SubmitEvent) {
		event.preventDefault();
		const value = region.value;
		if (!value || pending) return;
		if (!closeReason.trim()) {
			error = 'กรอกเหตุผลการปิดคำขอ';
			return;
		}
		pending = true;
		error = '';
		try {
			const basis = { expectedRevision: value.request.revision, reason: closeReason.trim() };
			const result = await closeBudgetRequest(id, { ...basis, requestId: intent.for(basis) });
			if (!disposed) {
				patch(result);
				closing = false;
			}
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<PageShell
	title={region.value?.request.title ?? 'รายละเอียดคำขอใช้เงิน'}
	backHref={returnTo}
	backLabel="กลับรายการคำขอ"
	contentClass="space-y-4"
>
	{#snippet actions()}{#if region.value}{@const detail =
				region.value}{#if detail.capabilities.canEdit}<Button
					variant="outline"
					disabled={pending}
					onclick={edit}>แก้ไขฉบับร่าง</Button
				>{/if}{#if detail.capabilities.canSubmit}<LoadingButton loading={pending} onclick={submit}
					>ส่งคำขอพิจารณา</LoadingButton
				>{/if}{#if detail.capabilities.canPay && $can.has(PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL)}<Button
					onclick={() => (paying = true)}>บันทึกการจ่ายเงิน</Button
				>{/if}{#if detail.capabilities.canClose}<Button
					variant="outline"
					disabled={pending}
					onclick={() => {
						closing = true;
						error = '';
					}}>ปิดคำขอ</Button
				>{/if}{/if}{/snippet}
	<Navigation />
	<section
		class="space-y-4"
		aria-label="รายละเอียดคำขอใช้เงิน"
		aria-busy={region.loading}
		data-testid="budget-request-detail"
	>
		{#if region.error}<PageState
				variant="error"
				title="โหลดคำขอไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => getBudgetRequest(id, { signal }))}
			/>{/if}
		{#if region.loading && !region.value}<PageSkeleton
				variant="form"
				rows={4}
			/>{:else if region.value}{@const detail = region.value}
			<div
				class="space-y-3 rounded-xl border border-primary/15 bg-gradient-to-r from-primary/5 to-card p-4"
			>
				<div class="flex flex-wrap items-center justify-between gap-2">
					<StatusBadge status={detail.request.status} /><Button
						variant="link"
						href={`/staff/budget/projects/${detail.request.projectId}`}>เปิดโครงการ</Button
					>
				</div>
				<dl class="grid grid-cols-2 gap-3 lg:grid-cols-4">
					{#each [{ label: 'จำนวนเงินคำขอ', value: detail.request.totalSatang }, { label: 'ผูกพัน', value: detail.request.reservedSatang }, { label: 'จ่ายแล้ว', value: detail.request.paidSatang }, { label: 'คงเหลือคำขอ', value: detail.request.remainingSatang }] as metric}<div
						>
							<dt class="text-sm text-muted-foreground">{metric.label}</dt>
							<dd class="font-semibold tabular-nums">{money(metric.value)} บาท</dd>
						</div>{/each}
				</dl>
			</div>
			<div class="overflow-x-auto rounded-lg border bg-card">
				<Table.Root
					><Table.Header
						><Table.Row
							><Table.Head>รายการ</Table.Head><Table.Head>แหล่งเงิน</Table.Head><Table.Head
								>หมวดรายจ่าย</Table.Head
							><Table.Head>จำนวนเงิน</Table.Head></Table.Row
						></Table.Header
					><Table.Body
						>{#each detail.lines as line (line.planLineId)}<Table.Row
								><Table.Cell class="min-w-44">{line.description}</Table.Cell><Table.Cell
									>{line.fundName}</Table.Cell
								><Table.Cell>{line.expenseCategoryName}</Table.Cell><Table.Cell
									class="text-right tabular-nums">{money(line.amountSatang)}</Table.Cell
								></Table.Row
							>{/each}</Table.Body
					></Table.Root
				>
			</div>
			<EvidenceList ids={detail.evidenceFileIds} resourceId={id} />{/if}
		{#if error && !closing}<PageState
				variant="error"
				title="ดำเนินการไม่สำเร็จ"
				description={error}
			/>{/if}
	</section>
	{#if region.value}<WorkflowPanel
			source={nextProcess ?? processSource}
			kind="requests"
			{id}
			revision={region.value.request.revision}
			canAct={region.value.capabilities.canAct && $can.has(PERMISSIONS.BUDGET_APPROVE_ASSIGNED)}
			onaction={async (input) => {
				const value = await actOnBudgetRequest(id, input);
				if (!disposed) patch(value);
			}}
		/>{/if}
	<section
		class="space-y-3"
		aria-label="ประวัติการจ่ายเงิน"
		aria-busy={payments.loading}
		data-testid="budget-payments"
	>
		<h2 class="font-semibold">การจ่ายเงินและคืนเงิน</h2>
		{#if payments.error}<PageState
				variant="error"
				title="โหลดการจ่ายเงินไม่สำเร็จ"
				description={payments.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => loadPayments()}
			/>{/if}{#if payments.loading && !payments.value}<TableSkeleton
				columns={4}
			/>{:else if payments.value?.items.length}<ul class="space-y-3">
				{#each payments.value.items as payment (payment.id)}<li
						class="space-y-3 rounded-xl border bg-card p-3 sm:p-4"
					>
						<div class="flex flex-wrap items-start justify-between gap-3">
							<div>
								<h3 class="font-medium">ใบสำคัญ {payment.voucherNumber}</h3>
								<p class="text-sm text-muted-foreground">
									{dateLabel(payment.paidOn)}{payment.note ? ` · ${payment.note}` : ''}
								</p>
							</div>
							<strong class="tabular-nums">{money(payment.totalSatang)} บาท</strong>
						</div>
						<ul class="space-y-1 text-sm">
							{#each payment.allocations as allocation, index (allocation.planLineId)}<li
									class="flex justify-between gap-3"
								>
									<span>{lineNames[allocation.planLineId] ?? `รายการที่ ${index + 1}`}</span><span
										class="tabular-nums">{money(allocation.amountSatang)} บาท</span
									>
								</li>{/each}
						</ul>
						<EvidenceList
							ids={payment.evidence.map((file) => file.fileId)}
							resourceId={id}
						/>{#if payment.refunds.length}<div class="space-y-2 border-t pt-2">
								{#each payment.refunds as refund (refund.id)}<div class="space-y-2 text-sm">
										<p>
											คืนเงิน {money(refund.totalSatang)} บาท · {dateLabel(refund.refundedOn)} · {refund.reason}
										</p>
										<EvidenceList
											ids={refund.evidence.map((file) => file.fileId)}
											resourceId={id}
										/>
									</div>{/each}
							</div>{/if}{#if region.value?.capabilities.canPay && $can.has(PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL)}<Button
								size="sm"
								variant="outline"
								onclick={() => (refund = payment)}>บันทึกคืนเงิน</Button
							>{/if}
					</li>{/each}
			</ul>
			<div class="flex justify-between">
				<Button
					size="sm"
					variant="outline"
					disabled={paymentOffset === 0 || payments.loading}
					onclick={() => loadPayments(Math.max(0, paymentOffset - 20))}>ก่อนหน้า</Button
				><Button
					size="sm"
					variant="outline"
					disabled={paymentOffset + 20 >= payments.value.total || payments.loading}
					onclick={() => loadPayments(paymentOffset + 20)}>ถัดไป</Button
				>
			</div>{:else if !payments.error}<PageState title="ยังไม่มีการจ่ายเงิน" />{/if}
	</section>
	<History kind="requests" {id} />
</PageShell>
<Dialog.Root bind:open={editing}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-2xl"
		><Dialog.Header
			><Dialog.Title>แก้ไขคำขอใช้เงิน</Dialog.Title><Dialog.Description
				>แก้ไขรายการและหลักฐานก่อนส่งพิจารณา</Dialog.Description
			></Dialog.Header
		>{#if editing}{#if editError}<PageState
					variant="error"
					title="โหลดแผนสำหรับแก้ไขไม่สำเร็จ"
					description={editError}
					actionLabel="ลองอีกครั้ง"
					onaction={edit}
				/>{:else if editLoading}<PageSkeleton
					variant="form"
					rows={4}
				/>{:else if region.value && workspace}<RequestForm
					{workspace}
					detail={region.value}
					onsaved={patch}
					oncancel={() => (editing = false)}
				/>{/if}{/if}</Dialog.Content
	></Dialog.Root
>
<Dialog.Root
	open={paying || refund !== null}
	onOpenChange={(open) => {
		if (!open) {
			paying = false;
			refund = null;
		}
	}}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-2xl"
		><Dialog.Header
			><Dialog.Title>{refund ? 'บันทึกคืนเงิน' : 'บันทึกการจ่ายเงิน'}</Dialog.Title
			><Dialog.Description>ระบุจำนวนเงินแยกตามรายการและแนบหลักฐาน</Dialog.Description
			></Dialog.Header
		>{#if (paying || refund) && region.value}<PaymentForm
				detail={region.value}
				payment={refund ?? undefined}
				{lineNames}
				oncancel={() => {
					paying = false;
					refund = null;
				}}
				onsaved={(value) => {
					patch(value);
					loadPayments();
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
<AlertDialog.Root bind:open={closing}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ปิดคำขอใช้เงิน</AlertDialog.Title><AlertDialog.Description
				>ยอดผูกพันที่ยังไม่จ่ายจะคืนเป็นงบประมาณที่ใช้ได้</AlertDialog.Description
			></AlertDialog.Header
		>
		<form onsubmit={close} class="space-y-4">
			<div class="space-y-2">
				<Label for="budget-close-reason">เหตุผลการปิดคำขอ *</Label><Textarea
					id="budget-close-reason"
					bind:value={closeReason}
					disabled={pending}
				/>
			</div>
			{#if error}<PageState
					variant="error"
					title="ปิดคำขอไม่สำเร็จ"
					description={error}
				/>{/if}<AlertDialog.Footer
				><Button
					type="button"
					variant="outline"
					disabled={pending}
					onclick={() => (closing = false)}>ยกเลิก</Button
				><LoadingButton type="submit" loading={pending}>ยืนยันปิดคำขอ</LoadingButton
				></AlertDialog.Footer
			>
		</form></AlertDialog.Content
	></AlertDialog.Root
>
