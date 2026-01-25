<script lang="ts">
	import type { MenuGroup } from '$lib/api/menu-admin';
	import { Card } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { GripVertical, CircleAlert } from 'lucide-svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		data: MenuGroup;
		children: Snippet;
		itemCount?: number;
		class?: string;
        // Group Drag Events
        onDragStart?: (e: DragEvent, group: MenuGroup) => void;
        onDragEnter?: (e: DragEvent, group: MenuGroup) => void;
        // Drop on Group (for items)
        onDragOver?: (e: DragEvent) => void;
        onDrop?: (e: DragEvent, group: MenuGroup) => void;
        draggable?: boolean;
	}

	let { 
        data: group, 
        children, 
        itemCount = 0, 
        class: className,
        onDragStart,
        onDragEnter,
        onDragOver,
        onDrop,
        draggable = true
    }: Props = $props(); 

</script>

<div
	role="group"
	{draggable}
	ondragstart={(e) => draggable && onDragStart?.(e, group)}
	ondragenter={(e) => draggable && onDragEnter?.(e, group)}
	ondragover={onDragOver}
	ondrop={(e) => onDrop?.(e, group)}
	class="relative {className || ''}"
>
	<Card class="p-4 bg-muted/40 hover:bg-muted/60 transition-colors">
		<div class="flex items-center gap-2 mb-3">
			{#if draggable}
				<div class="cursor-grab active:cursor-grabbing text-muted-foreground mr-2">
					<GripVertical class="h-5 w-5" />
				</div>
			{/if}

			<h3 class="text-lg font-semibold flex-1">{group.name}</h3>

			{#if group.name_en}
				<span class="text-sm text-muted-foreground">({group.name_en})</span>
			{/if}

			{#if group.code === 'other' && itemCount > 0}
				<Badge variant="destructive" class="gap-1">
					<CircleAlert class="h-3 w-3" />
					ต้องจัดกลุ่ม
				</Badge>
			{/if}
		</div>

		<div
			class="min-h-[60px] space-y-2 p-2 rounded-lg transition-all border-2 border-transparent border-dashed hover:border-muted-foreground/20"
		>
			{@render children()}
		</div>
	</Card>
</div>
