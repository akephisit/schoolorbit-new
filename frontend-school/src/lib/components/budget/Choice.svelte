<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import { Check, ChevronsUpDown } from '@lucide/svelte';
	let {
		id,
		label,
		value = $bindable(''),
		options,
		disabled = false,
		required = false,
		error = '',
		placeholder = 'เลือก',
		onchange
	}: {
		id: string;
		label: string;
		value?: string;
		options: { value: string; label: string; description?: string }[];
		disabled?: boolean;
		required?: boolean;
		error?: string;
		placeholder?: string;
		onchange?: (value: string) => void;
	} = $props();
	let open = $state(false),
		search = $state('');
	const selected = $derived(options.find((option) => option.value === value));
	const filtered = $derived(
		options.filter((option) =>
			`${option.label} ${option.description ?? ''}`
				.toLocaleLowerCase('th-TH')
				.includes(search.trim().toLocaleLowerCase('th-TH'))
		)
	);
</script>

<div class="min-w-0 space-y-2">
	<Label for={id}>{label}{required ? ' *' : ''}</Label>
	<Popover.Root bind:open onOpenChange={() => (search = '')}>
		<Popover.Trigger
			>{#snippet child({ props })}<Button
					{...props}
					{id}
					type="button"
					variant="outline"
					role="combobox"
					aria-label={label}
					aria-expanded={open}
					aria-required={required}
					aria-invalid={!!error}
					aria-describedby={error ? `${id}-error` : undefined}
					{disabled}
					class="w-full justify-between font-normal"
					><span class="min-w-0 truncate">{selected?.label ?? placeholder}</span><ChevronsUpDown
						class="size-4 shrink-0 opacity-50"
					/></Button
				>{/snippet}</Popover.Trigger
		>
		<Popover.Content class="w-[min(28rem,calc(100vw-2rem))] p-0" align="start" collisionPadding={8}>
			<Command.Root shouldFilter={false}
				><Command.Input
					aria-label={`ค้นหา${label}`}
					placeholder={`ค้นหา${label}`}
					bind:value={search}
				/><Command.List class="max-h-[min(18rem,50vh)]">
					{#if !options.length}<p role="status" class="p-3 text-sm text-muted-foreground">
							ยังไม่มีตัวเลือก
						</p>{:else if !filtered.length}<p
							role="status"
							class="p-3 text-sm text-muted-foreground"
						>
							ไม่พบตัวเลือกที่ตรงกับคำค้น
						</p>{:else}<Command.Group
							>{#each filtered as option (option.value)}<Command.Item
									value={option.value}
									onSelect={() => {
										value = option.value;
										open = false;
										onchange?.(value);
									}}
									><Check class={value === option.value ? 'opacity-100' : 'opacity-0'} /><span
										class="min-w-0 whitespace-normal break-words"
										>{option.label}{#if option.description}<span
												class="block text-xs text-muted-foreground">{option.description}</span
											>{/if}</span
									></Command.Item
								>{/each}</Command.Group
						>{/if}
				</Command.List></Command.Root
			>
		</Popover.Content>
	</Popover.Root>
	{#if error}<p id={`${id}-error`} class="text-sm text-destructive">{error}</p>{/if}
</div>
