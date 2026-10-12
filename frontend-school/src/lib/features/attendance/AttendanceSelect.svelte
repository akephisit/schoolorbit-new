<script lang="ts">
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	let {
		id,
		value = $bindable(''),
		options,
		label,
		placeholder = 'เลือก',
		disabled = false,
		onValueChange
	}: {
		id?: string;
		value?: string;
		options: { value: string; label: string; description?: string }[];
		label: string;
		placeholder?: string;
		disabled?: boolean;
		onValueChange?: (value: string) => void;
	} = $props();
	let search = $state('');
	const filtered = $derived(
		options.filter((o) => `${o.label} ${o.description ?? ''}`.includes(search))
	);
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
	<Select.Trigger {id} aria-label={label} class="w-full min-w-0"
		><span class="truncate">{current}</span></Select.Trigger
	>
	<Select.Content class="max-w-[calc(100vw-2rem)]">
		{#if options.length > 20}<Input
				aria-label={`ค้นหา ${label}`}
				placeholder="พิมพ์ค้นหา"
				bind:value={search}
				onkeydown={(e) => e.stopPropagation()}
			/>{/if}
		{#each filtered.slice(0, 100) as option (option.value)}<Select.Item
				value={option.value}
				label={option.label}
				><span class="flex min-w-0 flex-col items-start whitespace-normal text-left"
					><span>{option.label}</span>{#if option.description}<span
							class="text-xs text-muted-foreground">{option.description}</span
						>{/if}</span
				></Select.Item
			>{/each}
		{#if filtered.length > 100}<p class="p-2 text-sm text-muted-foreground">
				พิมพ์ค้นหาเพื่อดูรายชื่อเพิ่มเติม
			</p>{/if}
	</Select.Content>
</Select.Root>
