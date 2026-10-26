<script lang="ts">
	import type { CurriculumQuickAction } from '#lib/academic-core/curriculum-quick-actions.js';
	import type { CurriculumStructureWorkspace } from '#lib/api/academic-core.js';
	import CurriculumStructureMatrix from '#lib/components/academic-core/CurriculumStructureMatrix.svelte';
	let {
		workspace,
		gradeLevelId,
		termSlotIds,
		canManage = false,
		onQuickAction,
		busy = false
	}: {
		workspace: CurriculumStructureWorkspace;
		gradeLevelId: string;
		termSlotIds?: readonly string[];
		canManage?: boolean;
		onQuickAction?: (action: CurriculumQuickAction) => void;
		busy?: boolean;
	} = $props();
</script>

<div class="space-y-3">
	<h2 class="font-semibold">ภาพรวมทุกแผนการเรียน</h2>
	<CurriculumStructureMatrix
		{canManage}
		{onQuickAction}
		{busy}
		{workspace}
		{gradeLevelId}
		{termSlotIds}
	/>
</div>
