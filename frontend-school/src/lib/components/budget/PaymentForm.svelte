<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		createBudgetPayment,
		refundBudgetPayment,
		getBudgetRequest,
		listBudgetPayments,
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
	let evidenceEditor = $state<{ commit: () => Promise<void>; accept: () => void }>();
	import { ApiClientError } from '#lib/api/client.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	let {
		detail,
		payment,
		lineNames = {},
		oncancel,
		onreconciled,
		onsaved
	}: {
		detail: BudgetSpendingRequestDetail;
		payment?: BudgetPayment;
		lineNames?: Record<string, string>;
		oncancel: () => void;
		onreconciled: (value: BudgetSpendingRequestDetail) => void;
		onsaved: (value: BudgetSpendingRequestDetail) => void;
	} = $props();
	let expectedRevision = $state(untrack(() => detail.request.revision)),
		lines = $state(untrack(() => paymentLines(detail, payment)));
	let conflict = $state(false),
		reloading = $state(false),
		reviewed = $state(false),
		latest = $state<BudgetSpendingRequestDetail | null>(null),
		conflictReadError = $state('');
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
	const controller = new AbortController();
	const canUseLatest = $derived(
		!!latest &&
			(payment
				? ['approved', 'closed'].includes(latest.request.status)
				: latest.capabilities.canPay)
	);
	const total = $derived.by(() => {
		try {
			return sumSatang(Object.values(amounts).filter(Boolean).map(toSatang));
		} catch {
			return '0';
		}
	});
	onDestroy(() => {
		disposed = true;
		controller.abort();
	});
	async function reloadOriginalPayment() {
		if (!payment) return undefined;
		let offset = 0,
			total = Number.POSITIVE_INFINITY;
		while (offset < total) {
			const page = await listBudgetPayments(detail.request.id, {
				signal: controller.signal,
				query: { limit: 100, offset }
			});
			total = Math.min(total, page.total);
			const original = page.items.find((item) => item.id === payment.id);
			if (original) return original;
			if (page.limit <= 0) break;
			offset += page.limit;
		}
		throw new Error('ไม่พบการจ่ายเงินเดิม กรุณาตรวจสอบสิทธิ์และเปิดรายการอีกครั้ง');
	}
	async function reloadCurrent() {
		reloading = true;
		reviewed = false;
		conflictReadError = '';
		try {
			const [value, original] = await Promise.all([
				getBudgetRequest(detail.request.id, { signal: controller.signal }),
				reloadOriginalPayment()
			]);
			if (!disposed) {
				latest = value;
				lines = paymentLines(value, original);
				onreconciled(value);
			}
		} catch (cause) {
			if (!disposed) conflictReadError = message(cause);
		} finally {
			if (!disposed) reloading = false;
		}
	}
	function acceptCurrent() {
		if (!latest || !reviewed || !canUseLatest) return;
		expectedRevision = latest.request.revision;
		conflict = false;
		latest = null;
		error = '';
		intent.clear();
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending || uploading || conflict) return;
		error = '';
		try {
			if (!paidOn || !validDate(paidOn)) throw new Error('เลือกวันที่ทำรายการ');
			if (payment ? !reason.trim() : !voucher.trim())
				throw new Error(payment ? 'กรอกเหตุผลการคืนเงิน' : 'กรอกเลขที่ใบสำคัญ');
			const allocations = paymentAllocations(amounts, lines);
			pending = true;
			await evidenceEditor?.commit();
			if (disposed) return;
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
			if (!disposed) {
				evidenceEditor?.accept();
				onsaved(result);
			}
		} catch (cause) {
			if (!disposed) {
				error = message(cause);
				if (cause instanceof ApiClientError && cause.status === 409) {
					conflict = true;
					latest = null;
					reviewed = false;
				}
			}
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form onsubmit={save} class="w-full min-w-0 max-w-4xl space-y-6" data-testid="budget-payment-form">
	{#if conflict}<section
			class="space-y-3 rounded-xl border bg-card p-3 sm:p-4"
			aria-label="ทบทวนยอดล่าสุด"
		>
			<h2 class="font-semibold">ยอดหรือสถานะคำขอมีการเปลี่ยนแปลง</h2>
			<p class="text-sm">ข้อมูลที่กรอกยังอยู่ กรุณาโหลดยอดล่าสุดและตรวจสอบก่อนยืนยันอีกครั้ง</p>
			<Button type="button" variant="outline" disabled={reloading} onclick={reloadCurrent}
				>{reloading ? 'กำลังโหลดยอดล่าสุด…' : 'โหลดยอดล่าสุดและเก็บข้อมูลที่กรอก'}</Button
			>{#if conflictReadError}<PageState
					variant="error"
					title="โหลดยอดล่าสุดไม่สำเร็จ"
					description={conflictReadError}
				/>{/if}{#if latest}<p class="text-sm">
					คำขอฉบับล่าสุดที่ {latest.request.revision} · จ่ายแล้ว {money(latest.request.paidSatang)} บาท
				</p>
				{#if canUseLatest}<div class="flex items-start gap-2">
						<Checkbox id="budget-payment-review-latest" bind:checked={reviewed} /><Label
							for="budget-payment-review-latest"
							>ฉันตรวจสอบยอดคงเหลือล่าสุดและรายการที่ต้องการบันทึกแล้ว</Label
						>
					</div>
					<Button type="button" variant="outline" disabled={!reviewed} onclick={acceptCurrent}
						>ยืนยันใช้ยอดล่าสุด</Button
					>{:else}<PageState
						variant="permission"
						title="สถานะล่าสุดไม่อนุญาตให้ทำรายการนี้"
						description="ตรวจสอบสถานะคำขอกับผู้รับผิดชอบก่อนดำเนินการต่อ"
					/>{/if}{/if}
		</section>{/if}
	<section class="space-y-4" aria-label="ข้อมูลการทำรายการ">
		<h3 class="font-semibold">{payment ? 'ข้อมูลการคืนเงิน' : 'ข้อมูลการจ่ายเงิน'}</h3>
		<div class="grid gap-4 sm:grid-cols-2">
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
	</section>
	<fieldset class="min-w-0 space-y-4 border-t pt-6">
		<legend class="mb-2 font-semibold">จำนวนเงินแยกตามรายการ</legend
		>{#each lines as line, index (line.planLineId)}<div
				class="grid min-w-0 items-end gap-3 border-b pb-4 last:border-0 last:pb-0 sm:grid-cols-[minmax(0,1fr)_10rem]"
			>
				<Label for={`budget-payment-line-${line.planLineId}`}
					>{lineNames[line.planLineId] ?? `รายการที่ ${index + 1}`}<span
						class="block text-sm font-normal text-muted-foreground"
						>ยอดคงเหลือ{payment ? 'ที่คืนได้' : 'ที่จ่ายได้'}
						{money(line.remainingSatang)} บาท</span
					></Label
				><Input
					id={`budget-payment-line-${line.planLineId}`}
					aria-label={`จำนวนเงิน${payment ? 'คืน' : 'จ่าย'} ${lineNames[line.planLineId] ?? `รายการที่ ${index + 1}`}`}
					bind:value={amounts[line.planLineId]}
					class="text-right tabular-nums"
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
		bind:this={evidenceEditor}
		resourceId={detail.request.id}
		bind:ids={evidenceIds}
		bind:busy={uploading}
		disabled={pending}
	/>
	<div class="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-muted/40 p-4">
		<span class="text-sm text-muted-foreground">รวม{payment ? 'คืนเงิน' : 'จ่ายเงิน'}</span><strong
			class="text-lg tabular-nums">{money(total)} บาท</strong
		>
	</div>
	{#if error}<PageState variant="error" title="บันทึกไม่สำเร็จ" description={error} />{/if}
	<div class="flex flex-wrap justify-end gap-2 border-t pt-4">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending || uploading}
			>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={uploading || conflict}
			>{payment ? 'ยืนยันคืนเงิน' : 'บันทึกการจ่ายเงิน'}</LoadingButton
		>
	</div>
</form>
