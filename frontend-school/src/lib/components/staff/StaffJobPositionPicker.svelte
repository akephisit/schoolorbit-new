<script lang="ts">
	import { onDestroy } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { listStaffJobPositions, type StaffJobPositionSummary } from '#lib/api/personnel.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	let {
		selectableOnly = true,
		label,
		value = $bindable(null),
		selected = $bindable(null),
		disabled = false,
		emptyLabel = 'ยังไม่ระบุ',
		missingToken,
		onValueChange
	}: {
		selectableOnly?: boolean;
		label: string;
		value?: string | null;
		selected?: StaffJobPositionSummary | null;
		disabled?: boolean;
		emptyLabel?: string;
		missingToken?: string;
		onValueChange?: (value: string | null) => void;
	} = $props();
	const triggerId = $props.id();
	let open = $state(false),
		search = $state(''),
		loading = $state(false),
		error = $state('');
	let items = $state<StaffJobPositionSummary[]>([]);
	let refreshVersion = $state(0);
	const request = new LatestRequest();
	$effect.pre(() => {
		const context = [open, selectableOnly, search, disabled, refreshVersion] as const;
		if (!context[0] || context[3]) {
			request.abort();
			return;
		}
		const ticket = request.begin();
		loading = true;
		error = '';
		items = [];
		const timer = setTimeout(
			() => {
				void listStaffJobPositions(
					{ selectableOnly: context[1], search: context[2] || undefined, pageSize: 50 },
					{ signal: ticket.signal }
				).then(
					(result) => {
						if (request.isCurrent(ticket.revision)) {
							items = result.items;
							loading = false;
						}
					},
					(cause) => {
						if (request.isCurrent(ticket.revision)) {
							error = cause instanceof Error ? cause.message : 'โหลดตัวเลือกไม่สำเร็จ';
							loading = false;
						}
					}
				);
			},
			context[2] ? 200 : 0
		);
		return () => {
			clearTimeout(timer);
			request.abort();
		};
	});
	onDestroy(() => request.abort());
	function choose(item: StaffJobPositionSummary | null) {
		if (onValueChange) onValueChange(item?.id ?? null);
		else value = item?.id ?? null;
		selected = item;
		open = false;
		search = '';
	}
</script>

<div class="space-y-2 min-w-0">
	<Label for={triggerId}>{label}</Label>
	<Popover.Root bind:open>
		<Popover.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					id={triggerId}
					variant="outline"
					class="w-full min-w-0 justify-between border-input px-3 text-left font-normal"
					aria-label={label}
					{disabled}
				>
					<span class="truncate">
						{selected?.id === value
							? selected.name
							: value === missingToken
								? 'ยังไม่ระบุ'
								: value
									? 'รายการที่เลือก'
									: emptyLabel}{#if selected?.id === value && !selected.isActive}
							(ปิดใช้งาน){:else if selected?.id === value && !selected.isSelectable}(ตำแหน่งเดิม){/if}
					</span>
					<ChevronDown class="size-4 text-muted-foreground opacity-50" />
				</Button>
			{/snippet}
		</Popover.Trigger>
		<Popover.Content class="w-[min(22rem,calc(100vw-2rem))] space-y-3" align="start">
			<Input aria-label={`ค้นหา${label}`} placeholder={`ค้นหา${label}`} bind:value={search} />
			<Button variant="ghost" size="sm" onclick={() => choose(null)}>{emptyLabel}</Button>
			{#if missingToken}<Button
					variant="ghost"
					size="sm"
					onclick={() => {
						value = missingToken;
						selected = null;
						open = false;
					}}>ยังไม่ระบุ</Button
				>{/if}
			{#if loading}<PageSkeleton variant="table" rows={3} />{:else if error}<PageState
					variant="error"
					title={error}
					actionLabel="ลองอีกครั้ง"
					onaction={() => {
						refreshVersion++;
					}}
				/>{:else}
				<div class="max-h-64 overflow-y-auto" role="listbox" aria-label={`ตัวเลือก${label}`}>
					{#each items as item (item.id)}<button
							type="button"
							role="option"
							aria-selected={item.id === value}
							class="w-full rounded px-3 py-2 text-left text-sm hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
							onclick={() => choose(item)}
							>{item.name}{#if !item.isActive}
								(ปิดใช้งาน){:else if !item.isSelectable}
								(ตำแหน่งเดิม){/if}</button
						>{:else}<p class="p-3 text-sm text-muted-foreground">ไม่พบรายการ</p>{/each}
				</div>
				<p class="text-xs text-muted-foreground">
					แสดงสูงสุด 50 รายการ ใช้คำค้นเพื่อหารายการเพิ่มเติม
				</p>
			{/if}
		</Popover.Content>
	</Popover.Root>
</div>
