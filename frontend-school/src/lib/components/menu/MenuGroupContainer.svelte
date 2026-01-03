<script lang="ts">
	import type { MenuGroup } from '$lib/api/menu-admin';
	import { useSortable } from '@dnd-kit-svelte/sortable';
	import { Card } from '$lib/components/ui/card';
	import { Badge, badgeVariants } from '$lib/components/ui/badge';
	import { GripVertical, AlertCircle } from 'lucide-svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		data: MenuGroup;
		type: 'group';
		accepts: string[]; // ['item']
		children: Snippet;
		itemCount?: number;
		class?: string;
	}

	let { data: group, type, accepts, children, itemCount = 0, class: className }: Props = $props();

	const sortable = useSortable({
		id: group.id,
		data: { type, accepts }
	});

	const transform = $derived(sortable.transform);
	const transition = $derived(sortable.transition);
	const isDragging = $derived(sortable.isDragging);
</script>

<div
	use:sortable.setNodeRef
	class="relative {className || ''}"
	style:transform={transform
		? `translate3d(${Math.round(transform.x)}px, ${Math.round(transform.y)}px, 0)`
		: undefined}
	style:transition
	style:opacity={isDragging ? 0.5 : 1}
>
	<Card class="p-4">
		<div class="flex items-center gap-2 mb-3">
			<button
				use:sortable.setDraggableNodeRef
				{...sortable.attributes}
				{...sortable.listeners}
				class="cursor-grab active:cursor-grabbing"
			>
				<GripVertical class="h-5 w-5 text-muted-foreground" />
			</button>

			<h3 class="text-lg font-semibold flex-1">{group.name}</h3>

			{#if group.name_en}
				<span class="text-sm text-muted-foreground">({group.name_en})</span>
			{/if}

			{#if group.code === 'other' && itemCount > 0}
				<Badge variant="destructive" class="gap-1">
					<AlertCircle class="h-3 w-3" />
					ต้องจัดกลุ่ม
				</Badge>
			{/if}
		</div>

		<div class="min-h-[100px]">
			{@render children()}
		</div>
	</Card>
</div>
