<script lang="ts">
	import * as Select from '#lib/components/ui/select/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	let {
		id,
		label,
		value = $bindable(''),
		options,
		disabled = false,
		searchable = false,
		placeholder = 'เลือก',
		onchange
	}: {
		id: string;
		label: string;
		value?: string;
		options: { value: string; label: string }[];
		disabled?: boolean;
		searchable?: boolean;
		placeholder?: string;
		onchange?: (value: string) => void;
	} = $props();
	let search = $state('');
	const filtered = $derived(
		options.filter((option) =>
			option.label.toLocaleLowerCase().includes(search.toLocaleLowerCase())
		)
	);
</script>

<div class="min-w-0 space-y-2">
	<Label for={id}>{label}</Label>
	<Select.Root type="single" bind:value {disabled} onValueChange={onchange}>
		<Select.Trigger {id} class="w-full"
			><span class="truncate"
				>{options.find((option) => option.value === value)?.label ?? placeholder}</span
			></Select.Trigger
		>
		<Select.Content class="max-h-72">
			{#if searchable}<div class="p-2">
					<Input aria-label={`ค้นหา${label}`} bind:value={search} placeholder="ค้นหา" />
				</div>{/if}
			{#each filtered as option (option.value)}<Select.Item value={option.value}
					>{option.label}</Select.Item
				>{/each}
			{#if !filtered.length}<p class="p-3 text-sm text-muted-foreground">
					{options.length ? 'ไม่พบผลการค้นหา' : 'ไม่มีตัวเลือก'}
				</p>{/if}
		</Select.Content>
	</Select.Root>
</div>
