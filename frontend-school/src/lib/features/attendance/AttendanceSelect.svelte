<script lang="ts">
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	let {
		value = $bindable(''),
		options,
		label,
		placeholder = 'เลือก',
		disabled = false,
		onValueChange
	}: {
		value?: string;
		options: { value: string; label: string }[];
		label: string;
		placeholder?: string;
		disabled?: boolean;
		onValueChange?: (value: string) => void;
	} = $props();
	let search = $state('');
	const filtered = $derived(options.filter((o) => o.label.includes(search)));
	const current = $derived(options.find((o) => o.value === value)?.label ?? placeholder);
</script>

<Select.Root
	type="single"
	bind:value
	{disabled}
	{onValueChange}
	onOpenChange={(open) => {
		if (!open) search = '';
	}}
>
	<Select.Trigger aria-label={label}>{current}</Select.Trigger>
	<Select.Content>
		{#if options.length > 20}<Input
				aria-label={`ค้นหา ${label}`}
				placeholder="พิมพ์ค้นหา"
				bind:value={search}
				onkeydown={(e) => e.stopPropagation()}
			/>{/if}
		{#each filtered.slice(0, 100) as option (option.value)}<Select.Item
				value={option.value}
				label={option.label}>{option.label}</Select.Item
			>{/each}
		{#if filtered.length > 100}<p class="p-2 text-sm text-muted-foreground">
				พิมพ์ค้นหาเพื่อดูรายชื่อเพิ่มเติม
			</p>{/if}
	</Select.Content>
</Select.Root>
