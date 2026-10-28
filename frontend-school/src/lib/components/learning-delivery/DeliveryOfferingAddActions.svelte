<script lang="ts">
	import type { DeliveryOfferingAddKind } from '#lib/academic/learning-delivery-page.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Plus } from '@lucide/svelte';
	let {
		onAdd,
		disabled = false
	}: { onAdd: (kind: DeliveryOfferingAddKind) => void; disabled?: boolean } = $props();
</script>

<div class="flex flex-wrap gap-2" aria-label="เพิ่มรายการในรุ่นแบบร่าง">
	<Button size="sm" variant="outline" {disabled} onclick={() => onAdd('basic')}
		><Plus class="size-4" />เพิ่มวิชาพื้นฐาน</Button
	>
	<Button size="sm" variant="outline" {disabled} onclick={() => onAdd('additional')}
		><Plus class="size-4" />เพิ่มวิชาเพิ่มเติม</Button
	>
	<Button size="sm" variant="outline" {disabled} onclick={() => onAdd('activity')}
		><Plus class="size-4" />เพิ่มกิจกรรมพัฒนาผู้เรียน</Button
	>
</div>
