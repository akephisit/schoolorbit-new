<script lang="ts">
	import type { Building } from '#lib/api/facility.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { Pencil, Trash2 } from '@lucide/svelte';
	let {
		items,
		canUpdate,
		canDelete,
		onedit,
		ondelete
	}: {
		items: Building[];
		canUpdate: boolean;
		canDelete: boolean;
		onedit: (item: Building) => void;
		ondelete: (item: Building) => void;
	} = $props();
</script>

{#snippet actions(item: Building)}
	<div class="flex shrink-0 items-center justify-end gap-1">
		{#if canUpdate}<Button
				variant="ghost"
				size="icon-sm"
				aria-label={`แก้ไขอาคาร ${item.name_th}`}
				onclick={() => onedit(item)}><Pencil class="size-4" /></Button
			>{/if}
		{#if canDelete}<Button
				variant="ghost"
				size="icon-sm"
				class="text-destructive"
				aria-label={`ลบอาคาร ${item.name_th}`}
				onclick={() => ondelete(item)}><Trash2 class="size-4" /></Button
			>{/if}
	</div>
{/snippet}

<div
	class="hidden overflow-hidden rounded-lg border bg-card md:block"
	data-testid="facility-buildings-table"
>
	<Table.Root class="min-w-[38rem] table-fixed">
		<Table.Header
			><Table.Row
				><Table.Head class="w-24">รหัส</Table.Head><Table.Head>ชื่ออาคาร</Table.Head><Table.Head
					>รายละเอียด</Table.Head
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
	aria-label="รายการอาคาร"
	data-testid="facility-buildings-list"
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
			{#if item.description}<p class="break-words text-sm text-muted-foreground">
					{item.description}
				</p>{/if}
		</li>{/each}
</ul>
