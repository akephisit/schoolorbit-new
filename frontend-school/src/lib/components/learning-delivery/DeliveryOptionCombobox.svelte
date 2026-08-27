<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import * as Command from '$lib/components/ui/command';
	import * as Popover from '$lib/components/ui/popover';
	import { cn } from '$lib/utils';
	import { Check, ChevronsUpDown } from 'lucide-svelte';

	interface Option {
		id: string;
		label: string;
		description?: string;
	}

	let {
		value = $bindable(''),
		options,
		placeholder = 'เลือกข้อมูล',
		searchPlaceholder = 'ค้นหา...',
		ariaLabel = placeholder,
		disabled = false
	}: {
		value?: string;
		options: Option[];
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
			`${option.label} ${option.description ?? ''}`.toLocaleLowerCase('th-TH').includes(query)
		);
	});

	function select(optionId: string) {
		value = optionId;
		open = false;
		search = '';
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
				<span class={cn('truncate', !selected && 'text-muted-foreground')}>
					{selected?.label ?? placeholder}
				</span>
				<ChevronsUpDown class="ms-2 size-4 shrink-0 opacity-50" />
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-[--bits-popover-trigger-width] p-0" align="start">
		<Command.Root shouldFilter={false}>
			<Command.Input bind:value={search} placeholder={searchPlaceholder} />
			<Command.List class="max-h-72">
				{#if filtered.length === 0}
					<Command.Empty>ไม่พบข้อมูล</Command.Empty>
				{:else}
					<Command.Group>
						{#each filtered as option (option.id)}
							<Command.Item
								value={`${option.label} ${option.description ?? ''}`}
								onSelect={() => select(option.id)}
							>
								<Check
									class={cn('size-4 shrink-0', value === option.id ? 'opacity-100' : 'opacity-0')}
								/>
								<div class="min-w-0 flex-1">
									<p class="truncate">{option.label}</p>
									{#if option.description}
										<p class="truncate text-xs text-muted-foreground">{option.description}</p>
									{/if}
								</div>
							</Command.Item>
						{/each}
					</Command.Group>
				{/if}
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
