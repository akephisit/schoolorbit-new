<script lang="ts">
	import { Plus, Trash2 } from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import {
		EDUCATION_LEVEL_LABELS,
		newStaffEducation,
		type StaffEducationDraft
	} from '#lib/forms/staff-personnel.js';
	let {
		value = $bindable(),
		errors = {},
		disabled = false
	}: {
		value: StaffEducationDraft[];
		errors?: Record<string, string>;
		disabled?: boolean;
	} = $props();
	let removing = $state<string | null>(null);
	function remove(key: string) {
		value = value.filter((row) => row.key !== key);
		removing = null;
	}
</script>

<section class="space-y-4" aria-label="วุฒิการศึกษา" data-testid="staff-education-fields">
	<div class="flex flex-wrap items-center justify-between gap-3">
		<div>
			<h3 class="text-sm font-semibold">วุฒิการศึกษา</h3>
			<p class="mt-1 text-xs text-muted-foreground">
				เพิ่มได้หลายรายการ หน้ารายละเอียดจะเรียงระดับวุฒิจากต่ำไปสูง
			</p>
		</div>
		<Button
			type="button"
			variant="outline"
			size="sm"
			{disabled}
			onclick={() => (value = [...value, newStaffEducation()])}
			><Plus class="size-4" aria-hidden="true" />เพิ่มวุฒิการศึกษา</Button
		>
	</div>
	{#each value as row, index (row.key)}
		<fieldset class="min-w-0 rounded-xl border p-4" {disabled}>
			<legend class="px-1 text-sm font-medium">วุฒิที่ {index + 1}</legend>
			<div class="mb-3 flex justify-end">
				<Button
					type="button"
					variant="ghost"
					size="icon-sm"
					aria-label={`ลบวุฒิที่ ${index + 1}`}
					onclick={() => (row.id ? (removing = row.key) : remove(row.key))}
					><Trash2 class="size-4" aria-hidden="true" /></Button
				>
			</div>
			<div class="grid gap-3 sm:grid-cols-3">
				<div class="space-y-2">
					<Label for={`education-${row.key}`}
						>ระดับวุฒิ{#if !row.id}<span class="text-destructive">*</span>{/if}</Label
					>
					<Select.Root
						type="single"
						{disabled}
						value={row.education_level ?? 'unspecified'}
						onValueChange={(next) => {
							if (Object.hasOwn(EDUCATION_LEVEL_LABELS, next))
								row.education_level = next as keyof typeof EDUCATION_LEVEL_LABELS;
						}}
					>
						<Select.Trigger
							id={`education-${row.key}`}
							class="w-full"
							aria-label={`ระดับวุฒิ รายการที่ ${index + 1}`}
							aria-invalid={Boolean(errors[`education.${row.key}.education_level`])}
							>{row.education_level
								? EDUCATION_LEVEL_LABELS[row.education_level]
								: 'เลือกระดับวุฒิ'}</Select.Trigger
						>
						<Select.Content
							>{#each Object.entries(EDUCATION_LEVEL_LABELS) as [key, label] (key)}<Select.Item
									value={key}>{label}</Select.Item
								>{/each}</Select.Content
						>
					</Select.Root>
					{#if errors[`education.${row.key}.education_level`]}<p class="text-xs text-destructive">
							{errors[`education.${row.key}.education_level`]}
						</p>{/if}
				</div>
				{#each [{ key: 'major', label: 'สาขาวิชา' }, { key: 'university', label: 'สถาบันการศึกษา' }] as field (field.key)}
					{@const key = field.key as 'major' | 'university'}
					<div class="space-y-2">
						<Label for={`${key}-${row.key}`}>{field.label}</Label><Input
							id={`${key}-${row.key}`}
							bind:value={row[key]}
							aria-invalid={Boolean(errors[`education.${row.key}.${key}`])}
						/>
						{#if errors[`education.${row.key}.${key}`]}<p class="text-xs text-destructive">
								{errors[`education.${row.key}.${key}`]}
							</p>{/if}
					</div>
				{/each}
			</div>
		</fieldset>
	{:else}<p class="rounded-lg border border-dashed px-4 py-5 text-sm text-muted-foreground">
			ยังไม่มีข้อมูลวุฒิการศึกษา
		</p>{/each}
</section>
<Dialog.Root
	open={removing !== null}
	onOpenChange={(open) => {
		if (!open) removing = null;
	}}
>
	<Dialog.Content
		><Dialog.Header
			><Dialog.Title>นำวุฒิออกจากรายการ?</Dialog.Title><Dialog.Description
				>รายการนี้จะถูกลบเมื่อบันทึกการเปลี่ยนแปลง คุณยังยกเลิกการแก้ไขเพื่อกลับไปใช้ข้อมูลเดิมได้</Dialog.Description
			></Dialog.Header
		><Dialog.Footer
			><Button type="button" variant="outline" onclick={() => (removing = null)}>ยกเลิก</Button
			><Button
				type="button"
				variant="destructive"
				onclick={() => {
					if (removing) remove(removing);
				}}>นำออก</Button
			></Dialog.Footer
		></Dialog.Content
	>
</Dialog.Root>
