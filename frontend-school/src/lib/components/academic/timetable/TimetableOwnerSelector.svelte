<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import { cn } from '#lib/utils.js';
	import { Check, ChevronsUpDown } from '@lucide/svelte';

	import type { TimetableBoardRow } from '#lib/academic/timetable/board-state.js';

	let {
		value,
		onValueChange,
		options,
		placeholder = 'เลือกข้อมูล',
		searchPlaceholder = 'ค้นหา...',
		ariaLabel = placeholder,
		disabled = false
	}: {
		value: string;
		onValueChange: (value: string) => void;
		options: TimetableBoardRow[];
		placeholder?: string;
		searchPlaceholder?: string;
		ariaLabel?: string;
		disabled?: boolean;
	} = $props();

	let open = $state(false);
	let search = $state('');
	let selected = $derived(options.find((option) => option.id === value));
	let filtered = $derived.by(() => {
		const query = search.trim().toLocaleLowerCase('th-TH');
		if (!query) return options;
		return options.filter((option) =>
			`${option.label} ${option.code}`.toLocaleLowerCase('th-TH').includes(query)
		);
	});

	function select(optionId: string) {
		onValueChange(optionId);
		open = false;
		search = '';
	}
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button
				{...props}
				type="button"
				variant="outline"
				role="combobox"
				aria-label={ariaLabel}
				aria-expanded={open}
				class="w-full justify-between font-normal"
				{disabled}
			>
				<span class={cn('min-w-0 truncate', !selected && 'text-muted-foreground')}>
					{selected?.label ?? placeholder}
				</span>
				<ChevronsUpDown class="ms-2 size-4 shrink-0 opacity-50" />
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-[var(--bits-popover-anchor-width)] p-0" align="start">
		<Command.Root shouldFilter={false}>
			<Command.Input bind:value={search} placeholder={searchPlaceholder} />
			<Command.List class="max-h-72">
				{#if filtered.length === 0}
					<Command.Empty>ไม่พบรายการที่ค้นหา</Command.Empty>
				{:else}
					<Command.Group>
						{#each filtered as option (option.id)}
							<Command.Item
								value={`${option.label} ${option.code}`}
								onSelect={() => select(option.id)}
							>
								<Check
									class={cn('size-4 shrink-0', value === option.id ? 'opacity-100' : 'opacity-0')}
								/>
								<div class="min-w-0 flex-1">
									<p class="truncate">{option.label}</p>
								</div>
							</Command.Item>
						{/each}
					</Command.Group>
				{/if}
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
