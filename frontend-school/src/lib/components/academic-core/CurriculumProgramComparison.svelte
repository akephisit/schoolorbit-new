<script lang="ts">
	import type { CurriculumStructureWorkspace } from '#lib/api/academic-core.js';
	import CurriculumStructureMatrix from '#lib/components/academic-core/CurriculumStructureMatrix.svelte';
	let {
		workspace,
		gradeLevelId,
		termSlotIds
	}: {
		workspace: CurriculumStructureWorkspace;
		gradeLevelId: string;
		termSlotIds?: readonly string[];
	} = $props();
</script>

<div class="space-y-3">
	<h2 class="font-semibold">ภาพรวมทุกแผนการเรียน</h2>
	<CurriculumStructureMatrix {workspace} {gradeLevelId} {termSlotIds} />
</div>
