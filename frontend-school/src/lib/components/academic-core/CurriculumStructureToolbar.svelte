<script lang="ts">
	import type { CurriculumStructureWorkspace } from '$lib/api/academic-core';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import * as Tabs from '$lib/components/ui/tabs';
	import { PencilLine, ShieldAlert } from 'lucide-svelte';

	let {
		workspace,
		viewMode = $bindable<'comparison' | 'document'>('comparison'),
		gradeLevelId = $bindable(''),
		studyProgramId = $bindable(''),
		canManage = false,
		onEdit
	}: {
		workspace: CurriculumStructureWorkspace;
		viewMode?: 'comparison' | 'document';
		gradeLevelId?: string;
		studyProgramId?: string;
		canManage?: boolean;
		onEdit: () => void;
	} = $props();

	let selectedGrade = $derived(
		workspace.gradeLevels.find((grade) => grade.id === gradeLevelId)?.name ?? 'เลือกระดับชั้น'
	);
	let selectedProgram = $derived(
		workspace.programs.find((program) => program.id === studyProgramId)?.nameTh ??
			'เลือกแผนการเรียน'
	);
</script>

<div class="rounded-xl border bg-card p-3 sm:p-4">
	<div class="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
		<div class="flex min-w-0 flex-col gap-3 sm:flex-row sm:items-center">
			<Tabs.Root bind:value={viewMode} class="w-full sm:w-auto">
				<Tabs.List class="grid w-full grid-cols-2 sm:w-auto">
					<Tabs.Trigger value="comparison">เทียบทุกแผน</Tabs.Trigger>
					<Tabs.Trigger value="document">เอกสารรายแผน</Tabs.Trigger>
				</Tabs.List>
			</Tabs.Root>

			<Select.Root type="single" bind:value={gradeLevelId}>
				<Select.Trigger class="w-full sm:w-56">{selectedGrade}</Select.Trigger>
				<Select.Content>
					{#each workspace.gradeLevels as grade (grade.id)}
						<Select.Item value={grade.id}>{grade.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>

			{#if viewMode === 'document'}
				<Select.Root type="single" bind:value={studyProgramId}>
					<Select.Trigger class="w-full sm:w-64">{selectedProgram}</Select.Trigger>
					<Select.Content>
						{#each workspace.programs as program (program.id)}
							<Select.Item value={program.id}>
								{program.nameTh}{program.isDefault ? ' · แผนเริ่มต้น' : ''}
							</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			{/if}
		</div>

		<div class="flex flex-wrap items-center gap-2">
			{#if workspace.validation.blockers.length > 0}
				<Badge variant="destructive" class="gap-1">
					<ShieldAlert class="size-3.5" />
					ต้องแก้ {workspace.validation.blockers.length} จุดก่อนเผยแพร่
				</Badge>
			{/if}
			{#if canManage && workspace.curriculumVersion.status === 'draft'}
				<Button onclick={onEdit}><PencilLine class="size-4" /> จัดโครงสร้าง</Button>
			{/if}
		</div>
	</div>
</div>
