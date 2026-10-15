<script lang="ts">
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	let {
		value = $bindable(''),
		id,
		label,
		language,
		disabled = false,
		invalid = false,
		describedBy
	}: {
		value: string;
		id: string;
		label: string;
		language: 'th' | 'en';
		disabled?: boolean;
		invalid?: boolean;
		describedBy?: string;
	} = $props();
	const titles = $derived(
		language === 'th'
			? [
					'นาย',
					'นาง',
					'นางสาว',
					'ดร.',
					'ผศ.',
					'รศ.',
					'ศ.',
					'ผศ.ดร.',
					'รศ.ดร.',
					'ศ.ดร.',
					'ว่าที่ร้อยตรี'
				]
			: ['Mr.', 'Mrs.', 'Ms.', 'Miss', 'Dr.', 'Prof.', 'Assoc. Prof.', 'Asst. Prof.']
	);
	let custom = $state(false);
	const isCustom = $derived(custom || Boolean(value && !titles.includes(value)));
</script>

<Select.Root
	type="single"
	{disabled}
	value={isCustom ? '__custom' : value || '__empty'}
	onValueChange={(next) => {
		custom = next === '__custom';
		value = next === '__empty' || next === '__custom' ? '' : next;
	}}
>
	<Select.Trigger
		{id}
		class="w-full"
		aria-label={label}
		aria-invalid={invalid}
		aria-describedby={describedBy}
	>
		{isCustom ? 'อื่น ๆ / Other' : value || (language === 'th' ? 'เลือกคำนำหน้า' : 'Select title')}
	</Select.Trigger>
	<Select.Content>
		<Select.Item value="__empty">{language === 'th' ? 'ยังไม่ระบุ' : 'Not specified'}</Select.Item>
		{#each titles as title (title)}<Select.Item value={title}>{title}</Select.Item>{/each}
		<Select.Item value="__custom">อื่น ๆ / Other</Select.Item>
	</Select.Content>
</Select.Root>
{#if isCustom}
	<Label for={`${id}-custom`} class="sr-only">ระบุ {label}</Label>
	<Input
		id={`${id}-custom`}
		bind:value
		{disabled}
		aria-invalid={invalid}
		aria-describedby={describedBy}
		placeholder={language === 'th' ? 'ระบุคำนำหน้า' : 'Enter title'}
	/>
{/if}
