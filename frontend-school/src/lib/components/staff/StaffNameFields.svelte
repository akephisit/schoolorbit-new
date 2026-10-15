<script lang="ts">
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import {
		preferredStaffName,
		type StaffNameDraft,
		type StaffNameField
	} from '#lib/forms/staff-name.js';
	let {
		value = $bindable(),
		errors = {},
		disabled = false
	}: {
		value: StaffNameDraft;
		errors?: Record<string, string>;
		disabled?: boolean;
	} = $props();
	const groups: {
		language: string;
		fields: { key: StaffNameField; label: string; placeholder: string }[];
	}[] = [
		{
			language: 'ชื่อภาษาไทย',
			fields: [
				{ key: 'title', label: 'คำนำหน้า (ไทย)', placeholder: 'เช่น นาย นางสาว ดร.' },
				{ key: 'first_name', label: 'ชื่อ (ไทย)', placeholder: 'ชื่อ' },
				{ key: 'last_name', label: 'นามสกุล (ไทย)', placeholder: 'นามสกุล' }
			]
		},
		{
			language: 'ชื่อภาษาอังกฤษ',
			fields: [
				{ key: 'title_en', label: 'Title (English)', placeholder: 'เช่น Mr. Ms. Dr.' },
				{ key: 'first_name_en', label: 'First name (English)', placeholder: 'First name' },
				{ key: 'last_name_en', label: 'Last name (English)', placeholder: 'Last name' }
			]
		}
	];
</script>

<div class="space-y-4" data-testid="staff-name-fields">
	<div>
		<p class="text-sm font-medium">ชื่อบุคลากร</p>
		<p id="staff-name-help" class="mt-1 text-xs text-muted-foreground">
			กรอกให้ครบทั้งคำนำหน้า ชื่อ และนามสกุลอย่างน้อยหนึ่งภาษา หากครบทั้งสองภาษา
			ระบบจะแสดงชื่อไทยก่อน
		</p>
	</div>
	{#each groups as group (group.language)}
		<fieldset class="space-y-3" {disabled}>
			<legend class="mb-2 text-sm font-medium">{group.language}</legend>
			<div class="grid gap-3 sm:grid-cols-3">
				{#each group.fields as field (field.key)}
					<div class="space-y-2">
						<Label for={`staff-${field.key}`}>{field.label}</Label>
						<Input
							id={`staff-${field.key}`}
							bind:value={value[field.key]}
							placeholder={field.placeholder}
							aria-invalid={Boolean(errors[field.key])}
							aria-describedby={errors[field.key] ? `staff-${field.key}-error` : 'staff-name-help'}
						/>
						{#if errors[field.key]}<p
								id={`staff-${field.key}-error`}
								class="text-xs text-destructive"
							>
								{errors[field.key]}
							</p>{/if}
					</div>
				{/each}
			</div>
		</fieldset>
	{/each}
	{#if errors.names}<p role="alert" class="text-sm text-destructive">{errors.names}</p>{/if}
	{#if preferredStaffName(value)}<p class="rounded-lg bg-primary/5 px-3 py-2 text-sm">
			<span class="text-muted-foreground">ชื่อที่แสดง:</span>
			<span class="font-medium break-words">{preferredStaffName(value)}</span>
		</p>{/if}
</div>
