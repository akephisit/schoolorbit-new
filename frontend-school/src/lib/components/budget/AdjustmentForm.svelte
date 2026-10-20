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
		loan,
		oncancel,
		onsaved
	}: {
		detail?: BudgetAdjustmentDetail;
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
		fiscalYearId = $state(untrack(() => detail?.fiscalYearId ?? loan?.fiscalYearId ?? '')),
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
		readError = $state(''),
		error = $state(''),
		pending = $state(false),
		disposed = false;
	const request = new LatestRequest(),
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
	const from = $derived(refs.find((item) => item.key === fromKey));
	const destinationOptions = $derived(
		refs.filter((item) => item.key !== fromKey && (!from || item.ref.fundId === from.ref.fundId))
	);
	async function load() {
		const ticket = request.begin();
		loading = true;
		readError = '';
		try {
			const [catalogValues, rows] = await Promise.all([
				getBudgetCatalogs({ signal: ticket.signal }),
				getBudgetDashboard({
					signal: ticket.signal,
					query: { fiscalYearId: fiscalYearId || undefined }
				})
			]);
			if (!disposed && request.isCurrent(ticket.revision)) {
				catalogs = catalogValues;
				dashboard = rows;
			}
		} catch (cause) {
			if (!disposed && request.isCurrent(ticket.revision)) readError = message(cause);
		} finally {
			if (!disposed && request.isCurrent(ticket.revision)) loading = false;
		}
	}
	$effect.pre(() => {
		untrack(() => {
			void load();
		});
	});
	onDestroy(() => {
		disposed = true;
		request.abort();
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

<form onsubmit={save} class="space-y-4" data-testid="budget-adjustment-form">
	{#if loading && !catalogs}<PageSkeleton variant="form" rows={4} />{/if}{#if readError}<PageState
			variant="error"
			title="โหลดตัวเลือกงบประมาณไม่สำเร็จ"
			description={readError}
			actionLabel="ลองอีกครั้ง"
			onaction={load}
		/>{/if}<Choice
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
		options={(catalogs?.fiscalYears ?? [])
			.filter((year) => year.active || year.id === fiscalYearId)
			.map((year) => ({ value: year.id, label: year.name }))}
		disabled={pending || !!loan || !!detail}
		onchange={() => {
			fromKey = '';
			toKey = '';
			void load();
		}}
	/>{#if kind !== 'add'}<Choice
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
	<div class="space-y-2">
		<Label for="budget-adjustment-amount">จำนวนเงิน (บาท) *</Label><Input
			id="budget-adjustment-amount"
			bind:value={amount}
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
	{#if error}<PageState variant="error" title="บันทึกการปรับงบไม่สำเร็จ" description={error} />{/if}
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" disabled={pending} onclick={oncancel}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={loading || !!readError}
			>บันทึกฉบับร่าง</LoadingButton
		>
	</div>
</form>
