<script lang="ts">
	import type { GradeLevelOption } from '$lib/api/academic-core';
	import { gradeLevelLabel, normalizeCatalogSearch } from '$lib/academic-core/catalog-presentation';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Command from '$lib/components/ui/command';
	import * as Popover from '$lib/components/ui/popover';
	import { ChevronsUpDown } from 'lucide-svelte';

	interface Props {
		value?: string[];
		options: GradeLevelOption[];
		disabled?: boolean;
		ariaLabel?: string;
	}

	let {
		value = $bindable([]),
		options,
		disabled = false,
		ariaLabel = 'เลือกระดับชั้น'
	}: Props = $props();

	let open = $state(false);
	let search = $state('');

	let selectedOptions = $derived(options.filter((option) => value.includes(option.id)));
	let filteredOptions = $derived.by(() => {
		const query = normalizeCatalogSearch(search);
		if (!query) return options;
		return options.filter((option) =>
			[option.name, option.short_name, option.code, option.level_type].some(
				(candidate) => candidate && normalizeCatalogSearch(candidate).includes(query)
			)
		);
	});
	let triggerLabel = $derived.by(() => {
		if (selectedOptions.length === 0) return 'ทุกระดับชั้น';
		if (selectedOptions.length <= 2) return selectedOptions.map(gradeLevelLabel).join(', ');
		return `เลือกแล้ว ${selectedOptions.length} ระดับ`;
	});

	function toggle(optionId: string) {
		value = value.includes(optionId)
			? value.filter((selectedId) => selectedId !== optionId)
			: [...value, optionId];
	}

	function selectAll() {
		value = options.map((option) => option.id);
	}

	function clearSelection() {
		value = [];
	}
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button
				type="button"
				variant="outline"
				role="combobox"
				aria-label={ariaLabel}
				aria-expanded={open}
				class="w-full justify-between font-normal"
				{disabled}
				{...props}
			>
				<span class="truncate">{triggerLabel}</span>
				<ChevronsUpDown class="ms-2 size-4 shrink-0 opacity-50" />
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-[--bits-popover-trigger-width] p-0" align="start">
		<Command.Root shouldFilter={false}>
			<Command.Input bind:value={search} placeholder="ค้นหาระดับชั้น..." />
			<Command.List class="max-h-64">
				{#if filteredOptions.length === 0}
					<Command.Empty>ไม่พบระดับชั้น</Command.Empty>
				{:else}
					<Command.Group>
						{#each filteredOptions as option (option.id)}
							<Command.Item
								value={`${option.code} ${option.name} ${option.short_name ?? ''}`}
								onSelect={() => toggle(option.id)}
							>
								<Checkbox
									checked={value.includes(option.id)}
									aria-label={`เลือกระดับชั้น ${option.name}`}
									class="pointer-events-none"
								/>
								<div class="min-w-0 flex-1">
									<p class="truncate">{option.name}</p>
									<p class="truncate text-xs text-muted-foreground">{option.code}</p>
								</div>
								<span class="shrink-0 text-xs font-medium text-muted-foreground">
									{gradeLevelLabel(option)}
								</span>
							</Command.Item>
						{/each}
					</Command.Group>
				{/if}
			</Command.List>
			<div class="flex items-center justify-between border-t p-2">
				<Button type="button" size="sm" variant="ghost" onclick={clearSelection}>
					ทุกระดับชั้น
				</Button>
				<Button type="button" size="sm" variant="ghost" onclick={selectAll}>เลือกทั้งหมด</Button>
			</div>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
