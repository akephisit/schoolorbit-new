<script lang="ts">
	import { LoaderCircle } from 'lucide-svelte';
	import { cn } from '$lib/utils';

	let {
		label = 'กำลังอัปเดตข้อมูล...',
		class: className
	}: {
		label?: string;
		class?: string;
	} = $props();
</script>

<div
	role="status"
	aria-label={label}
	class={cn(
		'pointer-events-none absolute right-3 top-3 z-20 flex items-center gap-2 rounded-lg border bg-background/90 px-3 py-2 text-xs text-muted-foreground shadow-sm backdrop-blur-sm',
		className
	)}
>
	<LoaderCircle class="size-3.5 animate-spin" aria-hidden="true" />
	<span>{label}</span>
</div>
