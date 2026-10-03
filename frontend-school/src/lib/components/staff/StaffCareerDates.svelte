<script lang="ts">
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { Input } from '$lib/components/ui/input';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Label } from '$lib/components/ui/label';
	import type { StaffCareerFactDraft } from '$lib/forms/staff-career';
	let {
		value = $bindable(),
		prefix,
		disabled = false,
		errors = {}
	}: {
		value: StaffCareerFactDraft<string>;
		prefix: string;
		disabled?: boolean;
		errors?: Record<string, string>;
	} = $props();
</script>

<div class="grid min-w-0 gap-4 sm:grid-cols-2">
	{#each [{ key: 'effectiveDate', label: 'วันที่มีผล' }, { key: 'orderDate', label: 'วันที่ออกคำสั่ง' }] as field (field.key)}
		<div class="space-y-2">
			<Label for={`${prefix}-${field.key}`}>{field.label}</Label>
			<DatePicker
				id={`${prefix}-${field.key}`}
				value={value[field.key as 'effectiveDate' | 'orderDate']}
				onValueChange={(date) => {
					value[field.key as 'effectiveDate' | 'orderDate'] = date ?? '';
				}}
				ariaLabel={field.label}
				placeholder="ยังไม่ระบุวันที่"
				clearable
				{disabled}
			/>
			{#if errors[`${prefix}.${field.key}`]}<p class="text-sm text-destructive" role="alert">
					{errors[`${prefix}.${field.key}`]}
				</p>{/if}
		</div>
	{/each}
	<div class="space-y-2 sm:col-span-2">
		<Label for={`${prefix}-orderNumber`}>เลขที่คำสั่ง</Label><Input
			id={`${prefix}-orderNumber`}
			bind:value={value.orderNumber}
			{disabled}
			placeholder="เช่น 12/2569"
			aria-invalid={Boolean(errors[`${prefix}.orderNumber`])}
			aria-describedby={errors[`${prefix}.orderNumber`] ? `${prefix}-orderNumber-error` : undefined}
		/>{#if errors[`${prefix}.orderNumber`]}<p
				id={`${prefix}-orderNumber-error`}
				class="text-sm text-destructive"
				role="alert"
			>
				{errors[`${prefix}.orderNumber`]}
			</p>{/if}
	</div>
	<div class="space-y-2 sm:col-span-2">
		<Label for={`${prefix}-note`}>หมายเหตุ</Label><Textarea
			id={`${prefix}-note`}
			bind:value={value.note}
			{disabled}
			rows={2}
			placeholder="ข้อมูลเพิ่มเติมตามเอกสาร"
			aria-invalid={Boolean(errors[`${prefix}.note`])}
			aria-describedby={errors[`${prefix}.note`] ? `${prefix}-note-error` : undefined}
		/>{#if errors[`${prefix}.note`]}<p
				id={`${prefix}-note-error`}
				class="text-sm text-destructive"
				role="alert"
			>
				{errors[`${prefix}.note`]}
			</p>{/if}
	</div>
	<p class="text-xs text-muted-foreground sm:col-span-2">
		ระบุวันที่ตามเอกสาร เว้นว่างได้หากยังไม่มีข้อมูล
	</p>
</div>
