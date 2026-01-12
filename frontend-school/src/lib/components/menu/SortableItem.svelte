<script lang="ts">
	import type { MenuItem } from '$lib/api/menu-admin';
	import { useSortable } from '@dnd-kit-svelte/sortable';
	import { CSS } from '@dnd-kit-svelte/utilities';
	import { Card } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { GripVertical, Pencil, Trash2, Eye, EyeOff } from 'lucide-svelte';

	interface Props {
		item: MenuItem;
		onEdit: (item: MenuItem) => void;
		onDelete: (item: MenuItem) => void;
	}

	let { item, onEdit, onDelete }: Props = $props();

	// Use sortable hook with type data
	const sortable = useSortable({
		id: item.id,
		data: { type: 'item' } // Add type for drag detection
	});

	// Generate style string for smooth transform animations
	const style = $derived(
		[
			sortable.transform.current
				? `transform: ${CSS.Transform.toString(sortable.transform.current)}`
				: '',
			sortable.transition.current ? `transition: ${sortable.transition.current}` : '',
			sortable.isDragging.current ? 'z-index: 1' : ''
		]
			.filter(Boolean)
			.join('; ')
	);
</script>

<!-- use:sortable.setNodeRef is a Svelte action -->
<div use:sortable.setNodeRef {style}>
	<Card
		class="p-4 transition-all {sortable.isDragging.current
			? 'opacity-50 shadow-lg ring-2 ring-primary scale-105'
			: 'hover:shadow-md'}"
	>
		<div class="flex items-center gap-3">
			<!-- Drag Handle -->
			<button
				type="button"
				{...sortable.attributes.current}
				{...sortable.listeners.current}
				class="cursor-grab active:cursor-grabbing text-muted-foreground hover:text-foreground touch-none"
				aria-label="Drag to reorder"
			>
				<GripVertical class="h-5 w-5" />
			</button>

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

				<!-- Status -->
				{#if item.is_active}
					<Badge variant="default" class="gap-1">
						<Eye class="h-3 w-3" />
						แสดง
					</Badge>
				{:else}
					<Badge variant="secondary" class="gap-1">
						<EyeOff class="h-3 w-3" />
						ซ่อน
					</Badge>
				{/if}

				<!-- Actions -->
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
