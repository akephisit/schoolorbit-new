<script lang="ts">
	import { useDroppable } from '@dnd-kit-svelte/core';
	import type { Snippet } from 'svelte';

	interface Props {
		id: string;
		data?: { accepts?: string[] };
		children: Snippet;
	}

	let { id, data, children }: Props = $props();

	const droppable = useDroppable({
		id,
		data: data || {}
	});
</script>

<div
	use:droppable.setNodeRef
	class:border-primary={droppable.isOver}
	class:border-2={droppable.isOver}
	class="transition-colors"
>
	{@render children()}
</div>
