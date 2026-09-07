<script lang="ts">
	import { activityOutcomeLabel, resultBlockerLabel } from '$lib/academic/results/presentation';
	import type {
		ActivityResultPreparationWorkspace,
		ActivityResultBatchInput
	} from '$lib/api/academicResults';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import { CheckCircle2, CircleAlert, LockKeyhole } from 'lucide-svelte';

	type ActivityOutcome = NonNullable<ActivityResultBatchInput['cells'][number]['outcome']>;

	let {
		workspace,
		busyStudentId = '',
		confirming = false,
		onselect,
		onconfirm
	}: {
		workspace: ActivityResultPreparationWorkspace;
		busyStudentId?: string;
		confirming?: boolean;
		onselect: (
			studentAcademicYearId: string,
			outcome: ActivityOutcome | null,
			rowVersion: number | null
		) => void;
		onconfirm: () => void;
	} = $props();

	let blankCount = $derived(workspace.students.filter((student) => student.outcome == null).length);
</script>

<section class="overflow-hidden rounded-xl border bg-card" aria-label="ประเมินผลกิจกรรม">
	<div class="flex flex-col gap-3 border-b bg-muted/20 px-4 py-3 sm:flex-row sm:items-center">
		<div class="min-w-0 flex-1">
			<div class="flex flex-wrap items-center gap-2">
				<h2 class="font-semibold">ผลกิจกรรม ผ / มผ</h2>
				{#if workspace.locked}
					<Badge variant="secondary"><LockKeyhole class="size-3" /> ล็อกผลแล้ว</Badge>
				{:else if workspace.confirmationIsCurrent}
					<Badge class="bg-emerald-100 text-emerald-800 hover:bg-emerald-100">
						<CheckCircle2 class="size-3" /> ยืนยันกลุ่มแล้ว
					</Badge>
				{:else}
					<Badge variant="outline"><CircleAlert class="size-3" /> รอยืนยัน</Badge>
				{/if}
			</div>
			<p class="mt-1 text-sm text-muted-foreground">
				{blankCount === 0 ? 'ประเมินนักเรียนครบทุกคนแล้ว' : `ผลกิจกรรมยังขาด ${blankCount} คน`}
			</p>
		</div>
		<LoadingButton
			loading={confirming}
			disabled={!workspace.canConfirm || workspace.locked || blankCount > 0}
			onclick={onconfirm}
		>
			{workspace.confirmationIsCurrent ? 'ยืนยันอีกครั้ง' : 'ยืนยันผลกิจกรรม'}
		</LoadingButton>
	</div>

	{#if workspace.blockers.length > 0 && blankCount === 0}
		<div class="border-b bg-amber-50 px-4 py-3 text-sm text-amber-900">
			{#each workspace.blockers as blocker (`${blocker.code}:${blocker.studentAcademicYearId ?? ''}`)}
				<p>{resultBlockerLabel(blocker)}</p>
			{/each}
		</div>
	{/if}

	<div class="overflow-x-auto">
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head>นักเรียน</Table.Head>
					<Table.Head class="w-56">ผลกิจกรรม</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each workspace.students as student (student.studentAcademicYearId)}
					<Table.Row>
						<Table.Cell class="font-medium">{student.displayName}</Table.Cell>
						<Table.Cell>
							{#if workspace.canManage && !workspace.locked}
								<Select.Root
									type="single"
									value={student.outcome ?? 'blank'}
									disabled={busyStudentId === student.studentAcademicYearId}
									onValueChange={(value) =>
										onselect(
											student.studentAcademicYearId,
											value === 'blank' ? null : (value as ActivityOutcome),
											student.rowVersion ?? null
										)}
								>
									<Select.Trigger class="w-full">
										{activityOutcomeLabel(student.outcome)}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="blank">ยังไม่ประเมิน</Select.Item>
										<Select.Item value="pass">ผ · ผ่าน</Select.Item>
										<Select.Item value="fail">มผ · ไม่ผ่าน</Select.Item>
									</Select.Content>
								</Select.Root>
							{:else}
								<Badge variant="outline">{activityOutcomeLabel(student.outcome)}</Badge>
							{/if}
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	</div>
</section>
