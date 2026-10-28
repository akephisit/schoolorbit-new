<script lang="ts">
	import {
		deliveryAlignmentLabels,
		deliveryAlignmentBadgeClass,
		deliveryAlignmentNeedsReview,
		type DeliveryOfferingAlignment
	} from '#lib/academic/homeroom-delivery.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	let {
		fact,
		reviewOnly = false
	}: { fact: DeliveryOfferingAlignment | undefined; reviewOnly?: boolean } = $props();
	const visibleStates = $derived(
		fact?.states.filter((entry) => !reviewOnly || deliveryAlignmentNeedsReview(entry.state)) ?? []
	);
	const unknown = $derived(
		!fact || (fact.states.length === 0 && fact.unlinkedReasons.length === 0)
	);
</script>

{#if visibleStates.length || fact?.unlinkedReasons.length || unknown}
	<div class="flex flex-wrap gap-1.5">
		{#if fact}
			{#each visibleStates as entry (entry.state)}<Badge
					variant="outline"
					class={deliveryAlignmentBadgeClass(entry.state)}
					title={entry.rooms.map((room) => room.name).join(', ')}
					>{deliveryAlignmentLabels[entry.state]} · {entry.rooms.length} ห้อง</Badge
				>{/each}
			{#if fact.unlinkedReasons.length}<Badge
					variant="outline"
					class="border-warning/40 text-warning">ยังเชื่อมห้องไม่ได้</Badge
				>{/if}
			{#if fact.states.length === 0 && fact.unlinkedReasons.length === 0}<span
					class="text-xs text-muted-foreground">ยังไม่มีผลเทียบในห้องที่เลือก</span
				>{/if}
		{:else}<span class="text-xs text-muted-foreground">ยังไม่มีผลเทียบรายห้อง</span>{/if}
	</div>
{/if}
