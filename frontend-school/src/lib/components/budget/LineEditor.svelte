<script lang="ts">
	import type { BudgetCatalogs } from '#lib/api/budget.js';
	import type { LineDraft } from '#lib/budget/drafts.js';
	import { lineErrors, previewTotal } from '#lib/budget/drafts.js';
	import { money } from '#lib/budget/money.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import Choice from './Choice.svelte';
	import FieldError from './FieldError.svelte';
	let {
		line = $bindable(),
		catalogs,
		submitted = false,
		disabled = false,
		onremove
	}: {
		line: LineDraft;
		catalogs: BudgetCatalogs;
		submitted?: boolean;
		disabled?: boolean;
		onremove: () => void;
	} = $props();
	const errors = $derived(submitted ? lineErrors(line) : {});
	const id = $derived(`budget-line-${line.key}`);
</script>

<section
	class="space-y-3 rounded-lg border bg-card p-3"
	aria-label={line.description || 'รายการค่าใช้จ่ายใหม่'}
>
	<div class="flex items-start justify-between gap-3">
		<p class="text-sm font-medium">{line.supplyItemId ? 'พัสดุจากคลัง' : 'รายการกำหนดเอง'}</p>
		<Button
			type="button"
			variant="ghost"
			size="sm"
			{disabled}
			onclick={onremove}
			aria-label={`ลบรายการ ${line.description || 'ใหม่'}`}>ลบรายการ</Button
		>
	</div>
	<div class="space-y-2">
		<Label for={`${id}-description`}>ชื่อสินค้า / บริการ *</Label><Input
			id={`${id}-description`}
			bind:value={line.description}
			disabled={disabled || !!line.supplyItemId}
			aria-invalid={!!errors.description}
			aria-describedby={`${id}-description-error`}
		/><FieldError id={`${id}-description-error`} message={errors.description} />
	</div>
	<div class="grid gap-3 sm:grid-cols-3">
		<div class="space-y-2">
			<Label for={`${id}-quantity`}>จำนวน *</Label><Input
				id={`${id}-quantity`}
				bind:value={line.quantity}
				inputmode="decimal"
				{disabled}
				aria-invalid={!!errors.quantity}
				aria-describedby={`${id}-quantity-error`}
			/><FieldError id={`${id}-quantity-error`} message={errors.quantity} />
		</div>
		<div class="space-y-2">
			<Label for={`${id}-unit`}>หน่วยนับ *</Label><Input
				id={`${id}-unit`}
				bind:value={line.unit}
				disabled={disabled || !!line.supplyItemId}
				aria-invalid={!!errors.unit}
				aria-describedby={`${id}-unit-error`}
			/><FieldError id={`${id}-unit-error`} message={errors.unit} />
		</div>
		<div class="space-y-2">
			<Label for={`${id}-price`}>ราคาต่อหน่วย (บาท) *</Label><Input
				id={`${id}-price`}
				bind:value={line.price}
				inputmode="decimal"
				disabled={disabled || !!line.supplyItemId}
				aria-invalid={!!errors.price}
				aria-describedby={`${id}-price-error`}
			/><FieldError id={`${id}-price-error`} message={errors.price} />
		</div>
	</div>
	<div class="grid gap-3 sm:grid-cols-2">
		<Choice
			id={`${id}-fund`}
			label="แหล่งเงิน"
			required
			bind:value={line.fundId}
			options={catalogs.funds
				.filter((item) => item.active || item.id === line.fundId)
				.map((item) => ({ value: item.id, label: item.name }))}
			{disabled}
			error={errors.fundId}
		/><Choice
			id={`${id}-category`}
			label="หมวดรายจ่าย"
			required
			bind:value={line.expenseCategoryId}
			options={catalogs.expenseCategories
				.filter((item) => item.active || item.id === line.expenseCategoryId)
				.map((item) => ({ value: item.id, label: item.name }))}
			{disabled}
			error={errors.expenseCategoryId}
		/>
	</div>
	<div class="flex flex-wrap items-center justify-between gap-2">
		{#if line.supplyItemId}<Button
				type="button"
				variant="outline"
				size="sm"
				{disabled}
				onclick={() => {
					line.supplyItemId = null;
					line.supplyVersionId = null;
				}}>ใช้เป็นรายการกำหนดเอง</Button
			>{/if}
		<p class="ml-auto text-sm tabular-nums">รวม <strong>{money(previewTotal(line))} บาท</strong></p>
	</div>
</section>
