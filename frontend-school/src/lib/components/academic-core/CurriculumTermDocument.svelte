<script lang="ts">
	import type { CurriculumQuickAction } from '#lib/academic-core/curriculum-quick-actions.js';
	import type { CurriculumStructureWorkspace } from '#lib/api/academic-core.js';
	import CurriculumStructureMatrix from '#lib/components/academic-core/CurriculumStructureMatrix.svelte';
	let {
		workspace,
		studyProgramId,
		gradeLevelId,
		termSlotIds,
		canManage = false,
		onQuickAction,
		busy = false
	}: {
		workspace: CurriculumStructureWorkspace;
		studyProgramId: string;
		gradeLevelId: string;
		termSlotIds?: readonly string[];
		canManage?: boolean;
		onQuickAction?: (action: CurriculumQuickAction) => void;
		busy?: boolean;
	} = $props();
</script>

<article class="space-y-3">
	<header class="text-center">
		<p class="text-xs text-muted-foreground">โครงสร้างหลักสูตรสถานศึกษา</p>
		<h2 class="mt-1 text-lg font-semibold">
			{workspace.programs.find((program) => program.id === studyProgramId)?.nameTh ??
				'เลือกแผนการเรียน'}
		</h2>
		<p class="text-sm text-muted-foreground">{workspace.level.editionName}</p>
	</header>
	<CurriculumStructureMatrix
		{canManage}
		{onQuickAction}
		{busy}
		{workspace}
		{gradeLevelId}
		{termSlotIds}
		{studyProgramId}
	/>
</article>
