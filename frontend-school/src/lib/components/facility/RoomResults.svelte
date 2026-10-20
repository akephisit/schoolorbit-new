<script lang="ts">
	import type { Room } from '#lib/api/facility.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { Pencil, Trash2 } from '@lucide/svelte';
	let {
		items,
		roomTypes,
		canUpdate,
		canDelete,
		onedit,
		ondelete
	}: {
		items: Room[];
		roomTypes: { value: string; label: string }[];
		canUpdate: boolean;
		canDelete: boolean;
		onedit: (item: Room) => void;
		ondelete: (item: Room) => void;
	} = $props();
</script>

{#snippet actions(item: Room)}
	<div class="flex shrink-0 items-center justify-end gap-1">
		{#if canUpdate}<Button
				variant="ghost"
				size="icon-sm"
				aria-label={`แก้ไขห้อง ${item.name_th}`}
				onclick={() => onedit(item)}><Pencil class="size-4" /></Button
			>{/if}
		{#if canDelete}<Button
				variant="ghost"
				size="icon-sm"
				class="text-destructive"
				aria-label={`ลบห้อง ${item.name_th}`}
				onclick={() => ondelete(item)}><Trash2 class="size-4" /></Button
			>{/if}
	</div>
{/snippet}

<div
	class="hidden overflow-hidden rounded-lg border bg-card md:block"
	data-testid="facility-rooms-table"
>
	<Table.Root class="min-w-[36rem] table-fixed">
		<Table.Header
			><Table.Row
				><Table.Head>ชื่อห้อง</Table.Head><Table.Head class="w-32">ประเภท</Table.Head><Table.Head
					class="w-20 text-center">ความจุ</Table.Head
				><Table.Head>รายละเอียด</Table.Head>{#if canUpdate || canDelete}<Table.Head
						class="w-24 text-end">จัดการ</Table.Head
					>{/if}</Table.Row
			></Table.Header
		>
		<Table.Body
			>{#each items as item (item.id)}<Table.Row>
					<Table.Cell class="break-words whitespace-normal font-medium">{item.name_th}</Table.Cell>
					<Table.Cell
						><Badge variant="outline" class="whitespace-normal"
							>{roomTypes.find((type) => type.value === item.room_type)?.label ??
								item.room_type}</Badge
						></Table.Cell
					>
					<Table.Cell class="text-center tabular-nums">{item.capacity}</Table.Cell>
					<Table.Cell class="break-words whitespace-normal text-muted-foreground"
						>{item.description || '—'}</Table.Cell
					>
					{#if canUpdate || canDelete}<Table.Cell>{@render actions(item)}</Table.Cell>{/if}
				</Table.Row>{/each}</Table.Body
		>
	</Table.Root>
</div>
<ul
	class="divide-y rounded-lg border bg-card md:hidden"
	aria-label="รายการห้อง"
	data-testid="facility-rooms-list"
>
	{#each items as item (item.id)}<li class="space-y-2 p-3">
			<div class="flex items-start justify-between gap-2">
				<h2 class="min-w-0 break-words text-sm font-semibold">{item.name_th}</h2>
				{@render actions(item)}
			</div>
			<div class="flex flex-wrap items-center gap-2 text-xs">
				<Badge variant="outline"
					>{roomTypes.find((type) => type.value === item.room_type)?.label ?? item.room_type}</Badge
				><span class="tabular-nums">{item.capacity} คน</span>
			</div>
			{#if item.description}<p class="break-words text-sm text-muted-foreground">
					{item.description}
				</p>{/if}
		</li>{/each}
</ul>
