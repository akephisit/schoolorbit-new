<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		createBudgetPayment,
		refundBudgetPayment,
		type BudgetPayment,
		type BudgetSpendingRequestDetail
	} from '#lib/api/budget.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { toSatang, sumSatang, money } from '#lib/budget/money.js';
	import { paymentLines, paymentAllocations } from '#lib/budget/allocations.js';
	import { message, validDate } from '#lib/budget/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import EvidenceEditor from './EvidenceEditor.svelte';
	let {
		detail,
		payment,
		lineNames = {},
		oncancel,
		onsaved
	}: {
		detail: BudgetSpendingRequestDetail;
		payment?: BudgetPayment;
		lineNames?: Record<string, string>;
		oncancel: () => void;
		onsaved: (value: BudgetSpendingRequestDetail) => void;
	} = $props();
	const expectedRevision = untrack(() => detail.request.revision),
		lines = untrack(() => paymentLines(detail, payment));
	let paidOn = $state<string | undefined>(),
		voucher = $state(''),
		note = $state(''),
		reason = $state('');
	let amounts = $state<Record<string, string>>(
		untrack(() => Object.fromEntries(lines.map((line) => [line.planLineId, ''])))
	);
	let evidenceIds = $state<string[]>([]),
		uploading = $state(false),
		pending = $state(false),
		error = $state(''),
		disposed = false;
	const intent = new BudgetMutationIntent();
	const total = $derived.by(() => {
		try {
			return sumSatang(Object.values(amounts).filter(Boolean).map(toSatang));
		} catch {
			return '0';
		}
	});
	onDestroy(() => {
		disposed = true;
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending || uploading) return;
		error = '';
		try {
			if (!paidOn || !validDate(paidOn)) throw new Error('เลือกวันที่ทำรายการ');
			if (payment ? !reason.trim() : !voucher.trim())
				throw new Error(payment ? 'กรอกเหตุผลการคืนเงิน' : 'กรอกเลขที่ใบสำคัญ');
			const allocations = paymentAllocations(amounts, lines);
			pending = true;
			const common = { expectedRevision, allocations, evidenceFileIds: evidenceIds };
			let result: BudgetSpendingRequestDetail;
			if (payment) {
				const basis = { ...common, refundedOn: paidOn, reason: reason.trim() };
				result = await refundBudgetPayment(detail.request.id, payment.id, {
					...basis,
					requestId: intent.for(basis)
				});
			} else {
				const basis = {
					...common,
					paidOn,
					voucherNumber: voucher.trim(),
					note: note.trim() || null
				};
				result = await createBudgetPayment(detail.request.id, {
					...basis,
					requestId: intent.for(basis)
				});
			}
			if (!disposed) onsaved(result);
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form onsubmit={save} class="space-y-4" data-testid="budget-payment-form">
	<div class="grid gap-3 sm:grid-cols-2">
		<div class="space-y-2">
			<Label for="budget-payment-date">{payment ? 'วันที่คืนเงิน' : 'วันที่จ่ายเงิน'} *</Label
			><DatePicker
				id="budget-payment-date"
				bind:value={paidOn}
				ariaLabel={payment ? 'วันที่คืนเงิน' : 'วันที่จ่ายเงิน'}
				disabled={pending}
			/>
		</div>
		{#if !payment}<div class="space-y-2">
				<Label for="budget-payment-voucher">เลขที่ใบสำคัญ *</Label><Input
					id="budget-payment-voucher"
					bind:value={voucher}
					disabled={pending}
				/>
			</div>{/if}
	</div>
	<fieldset class="space-y-3">
		<legend class="mb-2 text-sm font-medium">จำนวนเงินแยกตามรายการ</legend
		>{#each lines as line, index (line.planLineId)}<div
				class="grid items-end gap-3 sm:grid-cols-[1fr_12rem]"
			>
				<Label for={`budget-payment-line-${line.planLineId}`}
					>{lineNames[line.planLineId] ?? `รายการที่ ${index + 1}`}<span
						class="block text-xs text-muted-foreground"
						>ยอดคงเหลือ{payment ? 'ที่คืนได้' : 'ที่จ่ายได้'}
						{money(line.remainingSatang)} บาท</span
					></Label
				><Input
					id={`budget-payment-line-${line.planLineId}`}
					aria-label={`จำนวนเงิน${payment ? 'คืน' : 'จ่าย'} ${lineNames[line.planLineId] ?? `รายการที่ ${index + 1}`}`}
					bind:value={amounts[line.planLineId]}
					inputmode="decimal"
					disabled={pending}
					placeholder="0.00"
				/>
			</div>{/each}
	</fieldset>
	{#if payment}<div class="space-y-2">
			<Label for="budget-refund-reason">เหตุผลการคืนเงิน *</Label><Textarea
				id="budget-refund-reason"
				bind:value={reason}
				disabled={pending}
			/>
		</div>{:else}<div class="space-y-2">
			<Label for="budget-payment-note">หมายเหตุ</Label><Textarea
				id="budget-payment-note"
				bind:value={note}
				disabled={pending}
			/>
		</div>{/if}
	<EvidenceEditor
		resourceId={detail.request.id}
		bind:ids={evidenceIds}
		bind:busy={uploading}
		disabled={pending}
	/>
	<p class="text-right tabular-nums">
		รวม{payment ? 'คืนเงิน' : 'จ่ายเงิน'} <strong>{money(total)} บาท</strong>
	</p>
	{#if error}<PageState variant="error" title="บันทึกไม่สำเร็จ" description={error} />{/if}
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending || uploading}
			>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={uploading}
			>{payment ? 'ยืนยันคืนเงิน' : 'บันทึกการจ่ายเงิน'}</LoadingButton
		>
	</div>
</form>
