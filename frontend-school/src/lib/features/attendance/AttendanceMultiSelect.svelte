<script lang="ts">
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	let {
		value = $bindable([]),
		options,
		label,
		disabled = false
	}: {
		value?: string[];
		options: { value: string; label: string }[];
		label: string;
		disabled?: boolean;
	} = $props();
	let search = $state('');
	const filtered = $derived(options.filter((o) => o.label.includes(search)));
</script>

<Select.Root
	type="multiple"
	bind:value
	{disabled}
	onOpenChange={(open) => {
		if (!open) search = '';
	}}
>
	<Select.Trigger aria-label={label}
		>{value.length ? `เลือกแล้ว ${value.length} รายการ` : 'เลือกได้หลายรายการ'}</Select.Trigger
	>
	<Select.Content>
		<Input
			aria-label={`ค้นหา ${label}`}
			placeholder="พิมพ์ค้นหา"
			bind:value={search}
			onkeydown={(e) => e.stopPropagation()}
		/>
		{#each filtered.slice(0, 100) as option (option.value)}<Select.Item
				value={option.value}
				label={option.label}>{option.label}</Select.Item
			>{/each}
		{#if filtered.length > 100}<p class="p-2 text-sm text-muted-foreground">
				พิมพ์ค้นหาเพื่อดูรายชื่อเพิ่มเติม
			</p>{/if}
	</Select.Content>
</Select.Root>
