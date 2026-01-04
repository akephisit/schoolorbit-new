<script lang="ts">
	import type { MenuItem } from '$lib/api/menu-admin';
	import {
		DndContext,
		PointerSensor,
		KeyboardSensor,
		closestCenter,
		type DragEndEvent
	} from '@dnd-kit-svelte/core';
	import { SortableContext, verticalListSortingStrategy } from '@dnd-kit-svelte/sortable';
	import SortableItem from './SortableItem.svelte';

	interface Props {
		items: MenuItem[];
		groupName?: string;
		onReorder: (items: MenuItem[]) => Promise<void>;
		onEdit: (item: MenuItem) => void;
		onDelete: (item: MenuItem) => void;
	}

	let { items = $bindable(), groupName, onReorder, onEdit, onDelete }: Props = $props();

	// Sensors for drag interactions - use descriptor format with options
	const sensors = [
		{ sensor: PointerSensor, options: {} },
		{ sensor: KeyboardSensor, options: {} }
	];

	// Helper to swap array elements
	function arrayMove<T>(array: T[], from: number, to: number): T[] {
		const newArray = [...array];
		const [movedItem] = newArray.splice(from, 1);
		newArray.splice(to, 0, movedItem);
		return newArray;
	}

	async function handleDragEnd(event: DragEndEvent) {
		const { active, over } = event;

		if (!over || active.id === over.id) return;

		const oldIndex = items.findIndex((item) => item.id === active.id);
		const newIndex = items.findIndex((item) => item.id === over.id);

		if (oldIndex === -1 || newIndex === -1) return;

		// Reorder array using dnd-kit utility
		items = arrayMove(items, oldIndex, newIndex);

		// Update display_order and save
		const reorderedItems = items.map((item, index) => ({
			...item,
			display_order: index + 1
		}));

		items = reorderedItems;
		await onReorder(reorderedItems);
	}
</script>

<div class="space-y-2">
	{#if groupName}
		<h3 class="text-lg font-semibold text-foreground mb-3">{groupName}</h3>
	{/if}

	<DndContext {sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
		<SortableContext items={items.map((item) => item.id)} strategy={verticalListSortingStrategy}>
			<div class="space-y-2">
				{#each items as item (item.id)}
					<SortableItem {item} {onEdit} {onDelete} />
				{/each}
			</div>
		</SortableContext>
	</DndContext>
</div>
