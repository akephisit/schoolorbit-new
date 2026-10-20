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
	<Table.Root class="min-w-[46rem] table-fixed">
		<Table.Header
			><Table.Row
				><Table.Head class="w-20">รหัสห้อง</Table.Head><Table.Head>ชื่อห้อง</Table.Head><Table.Head
					class="w-32">ประเภท</Table.Head
				><Table.Head>อาคาร / ชั้น</Table.Head><Table.Head class="w-16 text-center"
					>ความจุ</Table.Head
				>{#if canUpdate || canDelete}<Table.Head class="w-24 text-right">จัดการ</Table.Head
					>{/if}</Table.Row
			></Table.Header
		>
		<Table.Body
			>{#each items as item (item.id)}<Table.Row>
					<Table.Cell class="break-words whitespace-normal text-xs tabular-nums"
						>{item.code || '—'}</Table.Cell
					>
					<Table.Cell class="whitespace-normal"
						><p class="break-words font-medium">{item.name_th}</p>
						{#if item.name_en}<p class="mt-1 break-words text-xs text-muted-foreground">
								{item.name_en}
							</p>{/if}</Table.Cell
					>
					<Table.Cell
						><Badge variant="outline" class="whitespace-normal"
							>{roomTypes.find((type) => type.value === item.room_type)?.label ??
								item.room_type}</Badge
						></Table.Cell
					>
					<Table.Cell class="whitespace-normal"
						><p class="break-words">{item.building_name || 'ไม่ระบุอาคาร'}</p>
						{#if item.floor !== null}<p class="mt-1 text-xs text-muted-foreground">
								ชั้น {item.floor}
							</p>{/if}</Table.Cell
					>
					<Table.Cell class="text-center tabular-nums">{item.capacity}</Table.Cell>
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
				<div class="min-w-0">
					<p class="break-words text-xs text-muted-foreground">{item.code || 'ไม่ระบุรหัส'}</p>
					<h2 class="mt-1 break-words text-sm font-semibold">{item.name_th}</h2>
					{#if item.name_en}<p class="mt-1 break-words text-xs text-muted-foreground">
							{item.name_en}
						</p>{/if}
				</div>
				{@render actions(item)}
			</div>
			<Badge variant="outline"
				>{roomTypes.find((type) => type.value === item.room_type)?.label ?? item.room_type}</Badge
			>
			<dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
				<dt class="text-muted-foreground">อาคาร / ชั้น</dt>
				<dd class="min-w-0 break-words">
					{item.building_name || 'ไม่ระบุอาคาร'}{#if item.floor !== null}
						· ชั้น {item.floor}{/if}
				</dd>
				<dt class="text-muted-foreground">ความจุ</dt>
				<dd class="tabular-nums">{item.capacity} คน</dd>
			</dl>
		</li>{/each}
</ul>
