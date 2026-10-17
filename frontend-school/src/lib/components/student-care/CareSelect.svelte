<script lang="ts">
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	let {
		id,
		label,
		value = '',
		options,
		disabled = false,
		onValueChange
	}: {
		id: string;
		label: string;
		value?: string;
		options: { value: string; label: string }[];
		disabled?: boolean;
		onValueChange: (value: string) => void;
	} = $props();
	const selected = $derived(options.find((option) => option.value === value)?.label ?? 'เลือก');
</script>

<div class="space-y-2">
	<Label for={id}>{label}</Label>
	<Select.Root type="single" {value} {disabled} {onValueChange}>
		<Select.Trigger {id} aria-label={label} class="w-full">
			<span class="truncate">{selected}</span>
		</Select.Trigger>
		<Select.Content class="max-w-[calc(100vw-2rem)]">
			{#each options as option (option.value)}
				<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>
			{/each}
			{#if !options.length}<p class="p-3 text-sm text-muted-foreground">ไม่มีตัวเลือก</p>{/if}
		</Select.Content>
	</Select.Root>
</div>
