<script lang="ts">
	import type { OrganizationUnit } from '#lib/api/staff.js';
	import {
		organizationPickerOptions,
		organizationUnitPath
	} from '#lib/features/staff/organization-hierarchy.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import { Check, ChevronsUpDown } from '@lucide/svelte';

	let {
		id,
		label,
		units,
		value = $bindable('')
	}: { id: string; label: string; units: OrganizationUnit[]; value?: string } = $props();
	let open = $state(false);
	let search = $state('');
	const options = $derived(organizationPickerOptions(units, search));
	const selected = $derived(units.find((unit) => unit.id === value));
</script>

<Popover.Root bind:open onOpenChange={() => (search = '')}>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button
				{...props}
				{id}
				type="button"
				variant="outline"
				role="combobox"
				aria-label={label}
				aria-expanded={open}
				class="h-auto min-h-9 w-full justify-between text-left font-normal"
			>
				<span class="min-w-0 whitespace-normal break-words">
					{selected ? organizationUnitPath(selected, units) : 'เลือกหน่วยงาน'}
				</span>
				<ChevronsUpDown class="size-4 shrink-0 opacity-50" />
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-[min(28rem,calc(100vw-2rem))] p-0" align="start">
		<Command.Root shouldFilter={false}>
			<Command.Input
				aria-label={`ค้นหา${label}`}
				placeholder="ค้นหาชื่อหน่วยงานหรือสังกัด..."
				bind:value={search}
			/>
			<Command.List class="max-h-[min(18rem,50vh)]">
				{#if units.length === 0}
					<p role="status" class="p-4 text-sm text-muted-foreground">ยังไม่มีหน่วยงานให้เลือก</p>
				{:else if options.length === 0}
					<p role="status" class="p-4 text-sm text-muted-foreground">ไม่พบหน่วยงานที่ตรงกับคำค้น</p>
				{:else}
					<Command.Group>
						{#each options as option (option.unit.id)}
							<Command.Item
								value={option.unit.id}
								onSelect={() => {
									value = option.unit.id;
									open = false;
								}}
							>
								<Check class={value === option.unit.id ? 'opacity-100' : 'opacity-0'} />
								<span class="min-w-0 whitespace-normal break-words">
									<span class="block">{option.unit.name}</span>
									{#if option.context}
										<span class="block text-xs text-muted-foreground">{option.context}</span>
									{/if}
								</span>
							</Command.Item>
						{/each}
					</Command.Group>
				{/if}
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
