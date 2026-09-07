<script lang="ts">
	import {
		COURSE_OUTCOME_OPTIONS,
		courseOutcomeSelectionLabel,
		resultBlockerLabel,
		type CourseOutcomeSelection
	} from '$lib/academic/results/presentation';
	import type { CourseResultPreparationWorkspace } from '$lib/api/academicResults';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import { CheckCircle2, CircleAlert, LockKeyhole } from 'lucide-svelte';

	let {
		workspace,
		busyStudentId = '',
		confirming = false,
		onselect,
		onconfirm
	}: {
		workspace: CourseResultPreparationWorkspace;
		busyStudentId?: string;
		confirming?: boolean;
		onselect: (
			studentAcademicYearId: string,
			selection: CourseOutcomeSelection,
			rowVersion: number | null
		) => void;
		onconfirm: () => void;
	} = $props();
</script>

<section class="overflow-hidden rounded-xl border bg-card" aria-label="เตรียมผลการเรียนรายห้อง">
	<div class="flex flex-col gap-3 border-b bg-muted/20 px-4 py-3 sm:flex-row sm:items-center">
		<div class="min-w-0 flex-1">
			<div class="flex flex-wrap items-center gap-2">
				<h2 class="font-semibold">ผลการเรียนที่คำนวณแล้ว</h2>
				{#if workspace.locked}
					<Badge variant="secondary"><LockKeyhole class="size-3" /> ล็อกผลแล้ว</Badge>
				{:else if workspace.confirmationIsCurrent}
					<Badge class="bg-emerald-100 text-emerald-800 hover:bg-emerald-100">
						<CheckCircle2 class="size-3" /> ยืนยันห้องนี้แล้ว
					</Badge>
				{:else}
					<Badge variant="outline"><CircleAlert class="size-3" /> รอยืนยัน</Badge>
				{/if}
			</div>
			<p class="mt-1 text-sm text-muted-foreground">
				ระบบคำนวณคะแนนและเกรดให้เอง ครูเลือกได้เฉพาะ ตามคะแนน, 0, ร หรือ มส
			</p>
		</div>
		<LoadingButton
			loading={confirming}
			disabled={!workspace.canConfirm || workspace.locked || workspace.blockers.length > 0}
			onclick={onconfirm}
		>
			{workspace.confirmationIsCurrent ? 'ยืนยันอีกครั้ง' : 'ยืนยันผลห้องนี้'}
		</LoadingButton>
	</div>

	{#if workspace.blockers.length > 0}
		<div class="border-b bg-amber-50 px-4 py-3 text-sm text-amber-900">
			<p class="font-medium">ยังยืนยันไม่ได้</p>
			<ul class="mt-1 list-disc space-y-1 pl-5">
				{#each workspace.blockers as blocker (`${blocker.code}:${blocker.assessmentPhaseId ?? ''}:${blocker.studentAcademicYearId ?? ''}`)}
					<li>{resultBlockerLabel(blocker)}</li>
				{/each}
			</ul>
		</div>
	{/if}

	<div class="overflow-x-auto">
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head class="min-w-56">นักเรียน</Table.Head>
					<Table.Head class="w-28 text-right">คะแนนรวม</Table.Head>
					<Table.Head class="w-28 text-center">เกรดคำนวณ</Table.Head>
					<Table.Head class="min-w-52">ผลที่เตรียม</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each workspace.students as student (student.studentAcademicYearId)}
					<Table.Row>
						<Table.Cell class="font-medium">{student.displayName}</Table.Cell>
						<Table.Cell class="text-right font-mono tabular-nums">
							{student.calculatedScore}
						</Table.Cell>
						<Table.Cell class="text-center font-semibold">
							{student.calculatedGrade ?? '—'}
						</Table.Cell>
						<Table.Cell>
							{#if workspace.canManage && !workspace.locked}
								<Select.Root
									type="single"
									value={student.selection}
									disabled={busyStudentId === student.studentAcademicYearId}
									onValueChange={(value) =>
										onselect(
											student.studentAcademicYearId,
											value as CourseOutcomeSelection,
											student.selectionRowVersion ?? null
										)}
								>
									<Select.Trigger class="w-full">
										{courseOutcomeSelectionLabel(student.selection)}
									</Select.Trigger>
									<Select.Content>
										{#each COURSE_OUTCOME_OPTIONS as option (option.value)}
											<Select.Item value={option.value}>{option.label}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							{:else}
								<Badge variant="outline">{courseOutcomeSelectionLabel(student.selection)}</Badge>
							{/if}
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	</div>
	{#if workspace.students.length === 0}
		<div class="p-8 text-center text-sm text-muted-foreground">ยังไม่มีนักเรียนในกลุ่มนี้</div>
	{/if}
</section>
