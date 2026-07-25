<script lang="ts">
	import { ChevronDown } from 'lucide-svelte';
	import { Badge } from '$lib/components/ui/badge';
	import * as Collapsible from '$lib/components/ui/collapsible';
	import * as Table from '$lib/components/ui/table';
	import {
		formatStaffExamDate,
		formatStaffExamTime,
		groupStaffInvigilatorRowsByDay,
		type StaffExamInvigilatorRenderRow,
		type StaffExamRoomAssignmentRecord
	} from '$lib/utils/staff-exam-schedule-view';
	import { cn } from '$lib/utils.js';

	interface Props {
		rows: StaffExamInvigilatorRenderRow[];
		currentStaffId: string;
	}

	let { rows, currentStaffId }: Props = $props();

	function roomLabel(assignment: StaffExamRoomAssignmentRecord): string {
		return [assignment.buildingName, assignment.roomName].filter(Boolean).join(' · ') || '-';
	}

	function assignmentTimeRanges(sessions: StaffExamRoomAssignmentRecord['sessions']): string {
		const ranges = [...sessions]
			.sort(
				(left, right) =>
					left.startsAt.localeCompare(right.startsAt) || left.endsAt.localeCompare(right.endsAt)
			)
			.map(
				(session) =>
					`${formatStaffExamTime(session.startsAt)}–${formatStaffExamTime(session.endsAt)}`
			);
		return [...new Set(ranges)].join(', ') || '-';
	}
</script>

<div class="hidden min-w-0 md:block">
	<Table.Root class="min-w-[860px]">
		<Table.Header>
			<Table.Row>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 w-44 text-center">
					วันสอบ
				</Table.Head>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 w-36 text-center">
					ชั้นเรียน
				</Table.Head>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 w-40 text-center">
					ห้องสอบ
				</Table.Head>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 w-56 text-center">
					ช่วงเวลาสอบจริง
				</Table.Head>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 min-w-60">
					กรรมการคุมสอบ
				</Table.Head>
			</Table.Row>
		</Table.Header>
		<Table.Body>
			{#each rows as row (row.assignment.assignmentId)}
				<Table.Row
					class={cn(
						row.dayGroupIndex % 2 === 1 && 'bg-muted/15',
						row.showDayCell && 'border-t-2',
						row.isCurrentUser && 'bg-primary/5'
					)}
				>
					{#if row.showDayCell}
						<Table.Cell
							rowspan={row.dayRowSpan}
							class="bg-muted/30 text-center align-top font-medium whitespace-normal"
						>
							<div>{formatStaffExamDate(row.assignment.examDate)}</div>
							{#if row.assignment.dayLabel}
								<div class="mt-1 text-xs text-muted-foreground">{row.assignment.dayLabel}</div>
							{/if}
						</Table.Cell>
					{/if}
					<Table.Cell class="text-center font-medium">
						{row.assignment.classroomName}
					</Table.Cell>
					<Table.Cell class="text-center whitespace-normal">
						{roomLabel(row.assignment)}
					</Table.Cell>
					<Table.Cell class="text-center font-mono whitespace-normal">
						{assignmentTimeRanges(row.assignment.sessions)}
					</Table.Cell>
					<Table.Cell class="whitespace-normal">
						{#if row.assignment.invigilators.length === 0}
							<span class="text-muted-foreground">ยังไม่กำหนด</span>
						{:else}
							<div class="flex flex-wrap gap-1.5">
								{#each row.assignment.invigilators as invigilator (invigilator.staffId)}
									<Badge variant={invigilator.staffId === currentStaffId ? 'default' : 'outline'}>
										{invigilator.displayName}
										{#if invigilator.staffId === currentStaffId}
											<span class="sr-only">ผู้ใช้ปัจจุบัน</span>
											<span aria-hidden="true"> · ฉัน</span>
										{/if}
									</Badge>
								{/each}
							</div>
						{/if}
					</Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
</div>

<div class="space-y-3 md:hidden">
	{#each groupStaffInvigilatorRowsByDay(rows) as group, index (group.examDate)}
		<Collapsible.Root open={index === 0} class="overflow-hidden rounded-xl border bg-card">
			<Collapsible.Trigger
				class="flex w-full items-center justify-between gap-3 p-4 text-left font-medium"
			>
				<span>
					{formatStaffExamDate(group.examDate)}
					{#if group.dayLabel}
						<span class="mt-0.5 block text-xs font-normal text-muted-foreground">
							{group.dayLabel}
						</span>
					{/if}
				</span>
				<span class="flex shrink-0 items-center gap-2 text-sm text-muted-foreground">
					{group.rows.length} ห้อง
					<ChevronDown class="size-4" aria-hidden="true" />
				</span>
			</Collapsible.Trigger>
			<Collapsible.Content class="divide-y border-t">
				{#each group.rows as row (row.assignment.assignmentId)}
					<div class={cn('grid gap-2 p-4 text-sm', row.isCurrentUser && 'bg-primary/5')}>
						<div class="font-medium">{row.assignment.classroomName}</div>
						<div>
							<span class="text-muted-foreground">ห้องสอบ:</span>
							{roomLabel(row.assignment)}
						</div>
						<div>
							<span class="text-muted-foreground">เวลา:</span>
							<span class="font-mono">{assignmentTimeRanges(row.assignment.sessions)}</span>
						</div>
						<div class="flex flex-wrap items-center gap-1.5">
							<span class="mr-1 text-muted-foreground">กรรมการ:</span>
							{#if row.assignment.invigilators.length === 0}
								<span>ยังไม่กำหนด</span>
							{:else}
								{#each row.assignment.invigilators as invigilator (invigilator.staffId)}
									<Badge variant={invigilator.staffId === currentStaffId ? 'default' : 'outline'}>
										{invigilator.displayName}
										{#if invigilator.staffId === currentStaffId}
											<span aria-hidden="true"> · ฉัน</span>
										{/if}
									</Badge>
								{/each}
							{/if}
						</div>
					</div>
				{/each}
			</Collapsible.Content>
		</Collapsible.Root>
	{/each}
</div>
