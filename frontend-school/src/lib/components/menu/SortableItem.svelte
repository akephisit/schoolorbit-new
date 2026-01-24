<script lang="ts">
	import type { MenuItem } from '$lib/api/menu-admin';
	import { Card } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { GripVertical, Pencil, Trash2, Eye, EyeOff } from 'lucide-svelte';

	interface Props {
		item: MenuItem;
		onEdit: (item: MenuItem) => void;
		onDelete: (item: MenuItem) => void;
		// Drag Events
		onDragStart?: (e: DragEvent, item: MenuItem) => void;
		onDragEnter?: (e: DragEvent, item: MenuItem) => void;
		onDragEnd?: (e: DragEvent) => void;
	}

	let { item, onEdit, onDelete, onDragStart, onDragEnter, onDragEnd }: Props = $props();

    // Visual state for generic dragging feedback if needed, 
    // but usually parent handles ordering visually.
</script>

<div
	role="listitem"
	draggable={true}
	ondragstart={(e) => onDragStart?.(e, item)}
	ondragenter={(e) => onDragEnter?.(e, item)}
	ondragend={(e) => onDragEnd?.(e)}
	class="touch-none group relative bg-background rounded-lg transition-all"
	style="touch-action: none;"
>
	<Card class="p-4 hover:shadow-md transition-all active:cursor-grabbing">
		<div class="flex items-center gap-3">
			<!-- Drag Handle -->
			<div
				class="cursor-grab active:cursor-grabbing text-muted-foreground hover:text-foreground touch-none"
			>
				<GripVertical class="h-5 w-5" />
			</div>

			<!-- Menu Info -->
			<div class="flex-1 min-w-0">
				<div class="flex items-center gap-2">
					<p class="font-medium text-foreground truncate">{item.name}</p>
					{#if item.name_en}
						<span class="text-sm text-muted-foreground truncate">({item.name_en})</span>
					{/if}
				</div>
				<div class="flex items-center gap-2 mt-1">
					<code class="text-xs bg-muted px-2 py-0.5 rounded">{item.path}</code>
					{#if item.required_permission}
						<Badge variant="secondary" class="text-xs">{item.required_permission}</Badge>
					{/if}
					{#if item.user_type}
						<Badge
							variant={item.user_type === 'staff'
								? 'default'
								: item.user_type === 'student'
									? 'outline'
									: 'secondary'}
							class="text-xs"
						>
							{item.user_type === 'staff'
								? '👔 Staff'
								: item.user_type === 'student'
									? '🎓 Student'
									: '👨‍👩‍👧 Parent'}
						</Badge>
					{/if}
				</div>
			</div>

			<!-- Order Badge & Actions -->
			<div class="flex items-center gap-2">
				<Badge variant="outline" class="font-mono">#{item.display_order}</Badge>

				{#if item.is_active}
					<Badge variant="default" class="gap-1">
						<Eye class="h-3 w-3" />
					</Badge>
				{:else}
					<Badge variant="secondary" class="gap-1">
						<EyeOff class="h-3 w-3" />
					</Badge>
				{/if}

				<Button size="sm" variant="outline" onclick={() => onEdit(item)}>
					<Pencil class="h-4 w-4" />
				</Button>
				<Button size="sm" variant="destructive" onclick={() => onDelete(item)}>
					<Trash2 class="h-4 w-4" />
				</Button>
			</div>
		</div>
	</Card>
</div>
