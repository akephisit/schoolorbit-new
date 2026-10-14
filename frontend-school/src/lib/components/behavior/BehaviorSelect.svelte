<script lang="ts">
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
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
	let search = $state('');
	const filtered = $derived(
		options.filter((o) => o.label.toLocaleLowerCase('th').includes(search.toLocaleLowerCase('th')))
	);
	const selected = $derived(options.find((o) => o.value === value)?.label ?? 'เลือก');
</script>

<div class="space-y-2">
	<Label for={id}>{label}</Label>
	<Select.Root
		type="single"
		{value}
		{disabled}
		{onValueChange}
		onOpenChange={(open) => {
			if (!open) search = '';
		}}
	>
		<Select.Trigger {id} aria-label={label} class="min-h-[44px] w-full"
			><span class="truncate">{selected}</span></Select.Trigger
		>
		<Select.Content class="max-w-[calc(100vw-2rem)]">
			{#if options.length > 20}<Input
					class="min-h-[44px]"
					aria-label={`ค้นหา ${label}`}
					bind:value={search}
					onkeydown={(event) => event.stopPropagation()}
				/>{/if}
			{#each filtered.slice(0, 100) as option (option.value)}<Select.Item
					class="min-h-[44px]"
					value={option.value}
					label={option.label}>{option.label}</Select.Item
				>{/each}
			{#if !filtered.length}<p class="p-3 text-sm text-muted-foreground">
					{options.length ? 'ไม่พบรายการที่ค้นหา' : 'ไม่มีตัวเลือก'}
				</p>{:else if filtered.length > 100}<p class="p-3 text-sm text-muted-foreground">
					พิมพ์ค้นหาเพื่อดูรายการเพิ่มเติม
				</p>{/if}
		</Select.Content>
	</Select.Root>
</div>
