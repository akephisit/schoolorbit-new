<script lang="ts">
	import type { DeliveryReviewStatus } from '#lib/academic/homeroom-delivery.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	let {
		statuses,
		showCounts = true,
		center = false
	}: {
		statuses: DeliveryReviewStatus[];
		showCounts?: boolean;
		center?: boolean;
	} = $props();
	const reviewStatuses = $derived(statuses.filter((status) => status.review));
	const tones = {
		success: 'border-success/35 text-success',
		warning: 'border-warning/40 text-warning',
		info: 'border-primary/25 text-primary',
		neutral: 'border-border text-muted-foreground'
	};
</script>

{#if reviewStatuses.length}
	<div
		class="flex flex-wrap items-center gap-1.5"
		class:justify-center={center}
		data-testid="delivery-status-badges"
	>
		{#each reviewStatuses as status (status.key)}
			<Badge
				variant="outline"
				class={`${tones[status.tone]} max-w-full whitespace-normal`}
				data-status={status.key}
			>
				{showCounts || status.unit === 'กลุ่ม'
					? `${status.label} ${status.count} ${status.unit}`
					: status.label}
			</Badge>
		{/each}
	</div>
{/if}
