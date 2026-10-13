<script lang="ts">
	import * as Select from '#lib/components/ui/select/index.js';
	let {
		id,
		value = '',
		options,
		label,
		disabled = false,
		onValueChange
	}: {
		id: string;
		value?: string;
		options: { value: string; label: string }[];
		label: string;
		disabled?: boolean;
		onValueChange: (value: string) => void;
	} = $props();
	const selected = $derived(options.find((o) => o.value === value)?.label ?? 'เลือก');
</script>

<Select.Root type="single" {value} {disabled} {onValueChange}
	><Select.Trigger {id} aria-label={label} class="min-h-11 w-full"
		><span class="truncate">{selected}</span></Select.Trigger
	><Select.Content
		>{#each options as option (option.value)}<Select.Item value={option.value} label={option.label}
				>{option.label}</Select.Item
			>{/each}</Select.Content
	></Select.Root
>
