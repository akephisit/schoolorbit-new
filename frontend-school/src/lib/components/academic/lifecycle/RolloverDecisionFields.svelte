<script lang="ts">
	import type {
		YearRolloverPreview,
		YearRolloverStudent
	} from '#lib/api/academic-year-management.js';
	import {
		changeOutcome,
		gradeLabel,
		rolloverOutcomeLabels,
		usesDestination,
		type RolloverDraft,
		type RolloverOutcome
	} from '#lib/academic/lifecycle/year-management.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	let {
		id,
		draft,
		student,
		preview,
		disabled = false,
		onchange
	}: {
		id: string;
		draft: RolloverDraft;
		student: YearRolloverStudent;
		preview: YearRolloverPreview;
		disabled?: boolean;
		onchange: (draft: RolloverDraft) => void;
	} = $props();
	const programs = $derived(
		preview.programs.filter(
			(program) =>
				!!draft.targetGradeLevelId && program.gradeLevelIds.includes(draft.targetGradeLevelId)
		)
	);
	const rooms = $derived(
		preview.homerooms.filter(
			(room) =>
				room.gradeLevelId === draft.targetGradeLevelId &&
				room.studyProgramId === draft.targetStudyProgramId
		)
	);
</script>

<div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
	<div class="space-y-1.5">
		<Label for={`${id}-outcome`}>ผลปีใหม่</Label><Select.Root
			type="single"
			value={draft.outcome}
			{disabled}
			onValueChange={(value) => onchange(changeOutcome(draft, value as RolloverOutcome, student))}
			><Select.Trigger id={`${id}-outcome`} class="w-full"
				><span>{draft.outcome ? rolloverOutcomeLabels[draft.outcome] : 'เลือกผล'}</span
				></Select.Trigger
			><Select.Content
				>{#each Object.entries(rolloverOutcomeLabels) as [value, label] (value)}<Select.Item {value}
						>{label}</Select.Item
					>{/each}</Select.Content
			></Select.Root
		>
	</div>
	{#if usesDestination(draft.outcome)}
		<div class="space-y-1.5">
			<Label for={`${id}-grade`}>ชั้นปลายทาง</Label><Select.Root
				type="single"
				value={draft.targetGradeLevelId ?? ''}
				{disabled}
				onValueChange={(value) =>
					onchange({
						...draft,
						targetGradeLevelId: value,
						targetStudyProgramId: null,
						targetHomeroomId: null
					})}
				><Select.Trigger id={`${id}-grade`} class="w-full"
					><span
						>{draft.targetGradeLevelId
							? gradeLabel(preview.grades.find((grade) => grade.id === draft.targetGradeLevelId))
							: 'เลือกชั้น'}</span
					></Select.Trigger
				><Select.Content
					>{#each preview.grades.filter((grade) => grade.isActive && (draft.outcome !== 'repeat' || grade.id === student.sourceGradeLevelId)) as grade (grade.id)}<Select.Item
							value={grade.id}>{gradeLabel(grade)}</Select.Item
						>{/each}</Select.Content
				></Select.Root
			>
		</div>
		<div class="space-y-1.5">
			<Label for={`${id}-program`}>แผนการเรียนปลายทาง</Label><Select.Root
				type="single"
				value={draft.targetStudyProgramId ?? ''}
				disabled={disabled || !draft.targetGradeLevelId}
				onValueChange={(value) =>
					onchange({ ...draft, targetStudyProgramId: value, targetHomeroomId: null })}
				><Select.Trigger id={`${id}-program`} class="w-full"
					><span class="truncate"
						>{programs.find((program) => program.id === draft.targetStudyProgramId)?.name ??
							'เลือกแผนการเรียน'}</span
					></Select.Trigger
				><Select.Content
					>{#each programs as program (program.id)}<Select.Item value={program.id}
							>{program.name} · {program.editionName}</Select.Item
						>{/each}</Select.Content
				></Select.Root
			>
		</div>
		<div class="space-y-1.5">
			<Label for={`${id}-room`}>ห้องปลายทาง</Label><Select.Root
				type="single"
				value={draft.targetHomeroomId ?? '__none__'}
				disabled={disabled || !draft.targetStudyProgramId}
				onValueChange={(value) =>
					onchange({ ...draft, targetHomeroomId: value === '__none__' ? null : value })}
				><Select.Trigger id={`${id}-room`} class="w-full"
					><span
						>{rooms.find((room) => room.id === draft.targetHomeroomId)?.name ??
							'ยังไม่จัดห้อง'}</span
					></Select.Trigger
				><Select.Content
					><Select.Item value="__none__">ยังไม่จัดห้อง</Select.Item
					>{#each rooms as room (room.id)}<Select.Item value={room.id}
							>{room.name}{room.capacity === null
								? ''
								: ` (${room.occupiedCount}/${room.capacity})`}</Select.Item
						>{/each}</Select.Content
				></Select.Root
			>
		</div>
	{/if}
	<div class="space-y-1.5 sm:col-span-2 xl:col-span-4">
		<Label for={`${id}-reason`}>หมายเหตุ</Label><Input
			id={`${id}-reason`}
			value={draft.reason ?? ''}
			maxlength={1000}
			{disabled}
			oninput={(event) => onchange({ ...draft, reason: event.currentTarget.value })}
		/>
	</div>
</div>
