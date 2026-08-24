<script lang="ts">
	import { ChevronDown } from 'lucide-svelte';
	import { Badge } from '$lib/components/ui/badge';
	import * as Collapsible from '$lib/components/ui/collapsible';
	import * as Table from '$lib/components/ui/table';
	import {
		formatStaffExamDate,
		formatStaffExamTime,
		groupStaffScheduleRowsByDay,
		type StaffExamScheduleRenderRow
	} from '$lib/utils/staff-exam-schedule-view';
	import { cn } from '$lib/utils.js';

	interface Props {
		rows: StaffExamScheduleRenderRow[];
	}

	let { rows }: Props = $props();

	function roomLabel(session: StaffExamScheduleRenderRow['session']): string {
		return [session.buildingName, session.roomName].filter(Boolean).join(' · ') || '-';
	}

	function invigilatorLabel(
		invigilators: StaffExamScheduleRenderRow['session']['invigilators']
	): string {
		return invigilators.map((invigilator) => invigilator.displayName).join(', ') || 'ยังไม่กำหนด';
	}
</script>

<div class="hidden min-w-0 md:block">
	<Table.Root class="min-w-[1040px]">
		<Table.Header>
			<Table.Row>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 w-44 text-center">
					วันสอบ
				</Table.Head>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 w-32 text-center">
					เวลา
				</Table.Head>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 min-w-52">วิชา</Table.Head>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10">ประเภทการสอบ</Table.Head>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 w-32 text-center">
					ชั้นเรียน
				</Table.Head>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 w-36 text-center">
					ห้องสอบ
				</Table.Head>
				<Table.Head scope="col" class="bg-muted sticky top-0 z-10 min-w-52">
					กรรมการคุมสอบ
				</Table.Head>
			</Table.Row>
		</Table.Header>
		<Table.Body>
			{#each rows as row (row.session.sessionId)}
				<Table.Row
					class={cn(row.dayGroupIndex % 2 === 1 && 'bg-muted/15', row.showDayCell && 'border-t-2')}
				>
					{#if row.showDayCell}
						<Table.Cell
							rowspan={row.dayRowSpan}
							class="bg-muted/30 text-center align-top font-medium whitespace-normal"
						>
							<div>{formatStaffExamDate(row.session.examDate)}</div>
							{#if row.session.dayLabel}
								<div class="mt-1 text-xs text-muted-foreground">{row.session.dayLabel}</div>
							{/if}
						</Table.Cell>
					{/if}
					{#if row.showTimeCell}
						<Table.Cell
							rowspan={row.timeRowSpan}
							class="text-center align-top font-mono whitespace-nowrap"
						>
							{formatStaffExamTime(row.session.startsAt)}–{formatStaffExamTime(row.session.endsAt)}
						</Table.Cell>
					{/if}
					<Table.Cell class="whitespace-normal">
						<div class="font-medium">{row.session.subjectName}</div>
						<div class="text-xs text-muted-foreground">{row.session.subjectCode}</div>
					</Table.Cell>
					<Table.Cell class="whitespace-normal">
						<Badge variant="secondary">{row.session.assessmentCategoryName}</Badge>
					</Table.Cell>
					<Table.Cell class="text-center whitespace-normal">
						<Badge variant="outline">{row.session.gradeLevelName}</Badge>
						<div class="mt-1">{row.session.homeroomName}</div>
					</Table.Cell>
					<Table.Cell class="text-center whitespace-normal">{roomLabel(row.session)}</Table.Cell>
					<Table.Cell class="whitespace-normal">
						{invigilatorLabel(row.session.invigilators)}
					</Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
</div>

<div class="space-y-3 md:hidden">
	{#each groupStaffScheduleRowsByDay(rows) as group, index (group.examDate)}
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
					{group.rows.length} รายการ
					<ChevronDown class="size-4" aria-hidden="true" />
				</span>
			</Collapsible.Trigger>
			<Collapsible.Content class="divide-y border-t">
				{#each group.rows as row (row.session.sessionId)}
					<div class="grid gap-2 p-4 text-sm">
						<div>
							<span class="text-muted-foreground">เวลา:</span>
							<span class="font-mono">
								{formatStaffExamTime(row.session.startsAt)}–{formatStaffExamTime(
									row.session.endsAt
								)}
							</span>
						</div>
						<div>
							<span class="text-muted-foreground">วิชา:</span>
							<span class="font-medium">{row.session.subjectName}</span>
							<span class="text-xs text-muted-foreground">({row.session.subjectCode})</span>
						</div>
						<div>
							<span class="text-muted-foreground">ชั้นเรียน:</span>
							{row.session.homeroomName}
						</div>
						<div>
							<span class="text-muted-foreground">ห้องสอบ:</span>
							{roomLabel(row.session)}
						</div>
						<div>
							<span class="text-muted-foreground">กรรมการ:</span>
							{invigilatorLabel(row.session.invigilators)}
						</div>
					</div>
				{/each}
			</Collapsible.Content>
		</Collapsible.Root>
	{/each}
</div>
