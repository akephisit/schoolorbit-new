<script lang="ts">
	import type { CalendarColorKeyItem } from '$lib/utils/calendar';

	let { items = [] }: { items?: CalendarColorKeyItem[] } = $props();
</script>

<div
	class="flex shrink-0 overflow-x-auto px-1 pb-1 text-xs text-muted-foreground sm:overflow-visible"
	aria-label="หมวดหมู่กิจกรรมในปฏิทิน"
>
	<div class="flex min-w-max items-center gap-4 sm:min-w-0 sm:flex-wrap">
		{#each items as item (item.id)}
			<span class="flex shrink-0 items-center gap-1.5 whitespace-nowrap">
				<span class="size-2 rounded-full" style:background-color={item.color} aria-hidden="true"
				></span>
				{item.name}
			</span>
		{/each}
	</div>
</div>
