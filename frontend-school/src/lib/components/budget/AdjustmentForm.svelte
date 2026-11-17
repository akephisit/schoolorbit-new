<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getBudgetCatalogs,
		getBudgetDashboard,
		createBudgetAdjustment,
		saveBudgetAdjustment,
		type BudgetCatalogs,
		type BudgetDashboard,
		type BudgetAdjustmentDetail,
		type BudgetAllocationRef,
		type SaveBudgetAdjustmentRequest
	} from '#lib/api/budget.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { adjustmentLabels } from '#lib/budget/presentation.js';
	import { money, positiveAmount, bahtInput } from '#lib/budget/money.js';
	import { message } from '#lib/budget/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { LoadingButton, PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import Choice from './Choice.svelte';
	let {
		detail,
		initialFiscalYearId = '',
		loan,
		oncancel,
		onsaved
	}: {
		detail?: BudgetAdjustmentDetail;
		initialFiscalYearId?: string;
		loan?: BudgetAdjustmentDetail;
		oncancel: () => void;
		onsaved: (value: BudgetAdjustmentDetail) => void;
	} = $props();
	function refKey(ref: BudgetAllocationRef | null | undefined) {
		return ref ? `${ref.projectId}|${ref.activityId}|${ref.fundId}|${ref.expenseCategoryId}` : '';
	}
	let kind = $state<SaveBudgetAdjustmentRequest['kind']>(
			untrack(() => detail?.kind ?? (loan ? 'repayment' : 'add'))
		),
		fiscalYearId = $state(
			untrack(() => detail?.fiscalYearId ?? loan?.fiscalYearId ?? initialFiscalYearId)
		),
		fromKey = $state(untrack(() => refKey(detail?.from ?? loan?.to))),
		toKey = $state(untrack(() => refKey(detail?.to ?? loan?.from))),
		amount = $state(
			untrack(() =>
				detail ? bahtInput(detail.amountSatang) : loan ? bahtInput(loan.loanOutstandingSatang) : ''
			)
		),
		reason = $state(untrack(() => detail?.reason ?? ''));
	let catalogs = $state<BudgetCatalogs | null>(null),
		dashboard = $state<BudgetDashboard | null>(null),
		loading = $state(true),
		catalogLoading = $state(true),
		readError = $state(''),
		catalogReadError = $state(''),
		error = $state(''),
		pending = $state(false),
		disposed = false;
	const request = new LatestRequest(),
		catalogRequest = new LatestRequest(),
		intent = new BudgetMutationIntent(),
		expectedRevision = untrack(() => detail?.revision ?? 0);
	const refs = $derived(
		(dashboard?.rows ?? []).map((row) => ({
			key: refKey(row),
			ref: {
				projectId: row.projectId,
				activityId: row.activityId,
				fundId: row.fundId,
				expenseCategoryId: row.expenseCategoryId
			},
			label: `${row.projectName} · ${row.activityTitle} · ${row.fundName} · ${row.expenseCategoryName}`,
			available: row.availableSatang
		}))
	);
	const selectedYear = $derived(catalogs?.fiscalYears.find((year) => year.id === fiscalYearId));
	const from = $derived(refs.find((item) => item.key === fromKey));
	const destinationOptions = $derived(
		refs.filter((item) => item.key !== fromKey && (!from || item.ref.fundId === from.ref.fundId))
	);
	async function loadCatalogs() {
		const ticket = catalogRequest.begin();
		catalogLoading = true;
		catalogReadError = '';
		try {
			const value = await getBudgetCatalogs({ signal: ticket.signal });
			if (!disposed && catalogRequest.isCurrent(ticket.revision)) catalogs = value;
		} catch (cause) {
			if (!disposed && catalogRequest.isCurrent(ticket.revision)) catalogReadError = message(cause);
		} finally {
			if (!disposed && catalogRequest.isCurrent(ticket.revision)) catalogLoading = false;
		}
	}
	async function load() {
		const ticket = request.begin();
		loading = true;
		readError = '';
		dashboard = null;
		try {
			const rows = await getBudgetDashboard({
				signal: ticket.signal,
				query: { fiscalYearId: fiscalYearId || undefined }
			});
			if (!disposed && request.isCurrent(ticket.revision)) dashboard = rows;
		} catch (cause) {
			if (!disposed && request.isCurrent(ticket.revision)) readError = message(cause);
		} finally {
			if (!disposed && request.isCurrent(ticket.revision)) loading = false;
		}
	}
	$effect.pre(() => {
		untrack(() => {
			void loadCatalogs();
			void load();
		});
	});
	onDestroy(() => {
		disposed = true;
		request.abort();
		catalogRequest.abort();
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		error = '';
		try {
			if (!fiscalYearId || !reason.trim()) throw new Error('เลือกปีงบประมาณและกรอกเหตุผล');
			const amountSatang = positiveAmount(amount);
			if (BigInt(amountSatang) <= 0n) throw new Error('จำนวนเงินต้องมากกว่า 0');
			const fromRef = kind === 'add' ? null : (from?.ref ?? (loan ? loan.to : null));
			const toRef =
				kind === 'reduce'
					? null
					: (refs.find((item) => item.key === toKey)?.ref ?? (loan ? loan.from : null));
			if ((kind !== 'add' && !fromRef) || (kind !== 'reduce' && !toRef))
				throw new Error('เลือกงบประมาณต้นทางและปลายทางให้ครบ');
			pending = true;
			const basis = {
				kind,
				fiscalYearId,
				from: fromRef,
				to: toRef,
				amountSatang,
				reason: reason.trim(),
				loanId: detail?.loanId ?? loan?.id ?? null
			};
			const value = detail
				? await saveBudgetAdjustment(detail.id, {
						...basis,
						expectedRevision,
						requestId: intent.for({ ...basis, expectedRevision })
					})
				: await createBudgetAdjustment({ ...basis, requestId: intent.for(basis) });
			if (!disposed) onsaved(value);
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form
	onsubmit={save}
	class="w-full min-w-0 max-w-4xl space-y-6"
	data-testid="budget-adjustment-form"
>
	{#if catalogLoading && !catalogs}<PageSkeleton variant="form" rows={2} />{/if}
	{#if catalogReadError}<PageState
			variant="error"
			title="โหลดปีงบประมาณไม่สำเร็จ"
			description={catalogReadError}
			actionLabel="ลองอีกครั้ง"
			onaction={loadCatalogs}
		/>{/if}
	<section class="space-y-4" aria-label="ข้อมูลการปรับงบ">
		<h3 class="font-semibold">ข้อมูลการปรับงบ</h3>
		<div class="grid gap-4 sm:grid-cols-2">
			<Choice
				id="budget-adjustment-kind"
				label="ประเภทรายการ"
				required
				bind:value={kind}
				options={Object.entries(adjustmentLabels)
					.filter(([value]) => value !== 'repayment' || !!loan || detail?.kind === 'repayment')
					.map(([value, label]) => ({ value, label }))}
				disabled={pending || !!loan || !!detail}
				onchange={() => {
					fromKey = '';
					toKey = '';
				}}
			/><Choice
				id="budget-adjustment-year"
				label="ปีงบประมาณ"
				required
				bind:value={fiscalYearId}
				options={[
					...(catalogs?.fiscalYears ?? [])
						.filter((year) => !year.isClosed || year.id === fiscalYearId)
						.map((year) => ({ value: year.id, label: year.name })),
					...(catalogs && fiscalYearId && !selectedYear
						? [
								{
									value: fiscalYearId,
									label: 'ไม่พบปีงบประมาณที่เลือก'
								}
							]
						: [])
				]}
				disabled={pending || !!loan || !!detail}
				onchange={() => {
					fromKey = '';
					toKey = '';
					void load();
				}}
			/>
		</div>
		{#if !detail && !loan && catalogs && fiscalYearId && (!selectedYear || selectedYear.isClosed)}<p
				class="text-sm text-warning"
				role="status"
			>
				{selectedYear?.isClosed
					? 'ปีงบประมาณที่เลือกปิดแล้ว เลือกปีที่เปิดใช้งานเพื่อสร้างรายการ'
					: 'ไม่พบปีงบประมาณที่เลือก กรุณาเลือกปีงบประมาณอีกครั้ง'}
			</p>{/if}
		<p class="text-sm text-muted-foreground">
			{kind === 'add'
				? 'เพิ่มวงเงินให้รายการงบประมาณปลายทาง'
				: kind === 'reduce'
					? 'ลดวงเงินจากรายการงบประมาณต้นทาง'
					: kind === 'loan'
						? 'ย้ายวงเงินชั่วคราวจากต้นทางไปปลายทาง และติดตามยอดยืมค้างคืน'
						: kind === 'repayment'
							? 'คืนวงเงินให้ต้นทางของรายการยืมเดิม'
							: 'ย้ายวงเงินจากต้นทางไปปลายทาง'}
		</p>
	</section>
	<section class="space-y-4 border-t pt-6" aria-label="งบประมาณที่เกี่ยวข้อง" aria-busy={loading}>
		<h3 class="font-semibold">งบประมาณที่เกี่ยวข้อง</h3>
		<p class="text-sm text-muted-foreground">
			เลือกจากรายการในแผนที่อนุมัติ {kind !== 'add' && kind !== 'reduce'
				? 'ต้นทางและปลายทางต้องใช้แหล่งเงินเดียวกัน'
				: 'ของปีงบประมาณนี้'}
		</p>
		{#if readError}<PageState
				variant="error"
				title="โหลดตัวเลือกงบประมาณไม่สำเร็จ"
				description={readError}
				actionLabel="ลองอีกครั้ง"
				onaction={load}
			/>{/if}
		{#if loading && !dashboard}<PageSkeleton variant="form" rows={2} />{/if}
		{#if kind !== 'add'}<Choice
				id="budget-adjustment-from"
				label="งบประมาณต้นทาง"
				required
				bind:value={fromKey}
				options={refs.map((item) => ({
					value: item.key,
					label: item.label,
					description: `ใช้ได้ ${money(item.available)} บาท`
				}))}
				disabled={pending || loading || !!loan}
				onchange={() => (toKey = '')}
			/>{/if}{#if kind !== 'reduce'}<Choice
				id="budget-adjustment-to"
				label="งบประมาณปลายทาง"
				required
				bind:value={toKey}
				options={(kind === 'add' ? refs : destinationOptions).map((item) => ({
					value: item.key,
					label: item.label
				}))}
				disabled={pending || loading || !!loan}
			/>{/if}{#if loan}<p class="text-sm text-muted-foreground">
				เงินยืมค้างคืน {money(loan.loanOutstandingSatang)} บาท
			</p>{/if}
	</section>
	<section class="space-y-4 border-t pt-6" aria-label="จำนวนเงินและเหตุผล">
		<h3 class="font-semibold">จำนวนเงินและเหตุผล</h3>
		<div class="w-full space-y-2 sm:max-w-xs">
			<Label for="budget-adjustment-amount">จำนวนเงิน (บาท) *</Label><Input
				id="budget-adjustment-amount"
				bind:value={amount}
				class="text-right tabular-nums"
				inputmode="decimal"
				disabled={pending}
			/>
		</div>
		<div class="space-y-2">
			<Label for="budget-adjustment-reason">เหตุผล *</Label><Textarea
				id="budget-adjustment-reason"
				bind:value={reason}
				disabled={pending}
			/>
		</div>
	</section>
	{#if error}<PageState variant="error" title="บันทึกการปรับงบไม่สำเร็จ" description={error} />{/if}
	<div class="flex flex-wrap justify-end gap-2 border-t pt-4">
		<Button type="button" variant="outline" disabled={pending} onclick={oncancel}>ยกเลิก</Button
		><LoadingButton
			type="submit"
			loading={pending}
			disabled={loading || catalogLoading || !!readError || !!catalogReadError}
			>บันทึกฉบับร่าง</LoadingButton
		>
	</div>
</form>
