<script lang="ts">
	import { CalendarDays, CheckCircle2, Clock3, DoorOpen } from 'lucide-svelte';
	import { PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import {
		formatStaffExamDate,
		formatStaffExamMinutes,
		formatStaffExamTime,
		type MyExamInvigilationItem,
		type MyExamInvigilationSummary
	} from '$lib/utils/staff-exam-schedule-view';

	interface Props {
		summary: MyExamInvigilationSummary;
	}

	let { summary }: Props = $props();
	let upcomingItems = $derived(summary.items.filter((item) => item.status === 'upcoming'));
	let completedItems = $derived(summary.items.filter((item) => item.status === 'completed'));

	function roomLabel(item: MyExamInvigilationItem): string {
		return [item.buildingName, item.roomName].filter(Boolean).join(' · ') || '-';
	}

	function timeBounds(item: MyExamInvigilationItem): string {
		if (!item.earliestStartsAt && !item.latestEndsAt) return '-';
		return `${formatStaffExamTime(item.earliestStartsAt ?? '')}–${formatStaffExamTime(
			item.latestEndsAt ?? ''
		)}`;
	}

	function subjectNames(item: MyExamInvigilationItem): string {
		return [...new Set(item.sessions.map((session) => session.subjectName))].join(', ') || '-';
	}
</script>

<div class="grid gap-3 sm:grid-cols-3">
	<Card.Root class="gap-0 py-0">
		<Card.Content class="flex items-center gap-3 p-4">
			<div class="rounded-lg bg-primary/10 p-2 text-primary">
				<CalendarDays class="size-5" aria-hidden="true" />
			</div>
			<div>
				<div class="text-2xl font-semibold">{summary.assignedDayCount}</div>
				<div class="text-xs text-muted-foreground">วันที่ได้รับมอบหมาย</div>
			</div>
		</Card.Content>
	</Card.Root>
	<Card.Root class="gap-0 py-0">
		<Card.Content class="flex items-center gap-3 p-4">
			<div class="rounded-lg bg-primary/10 p-2 text-primary">
				<DoorOpen class="size-5" aria-hidden="true" />
			</div>
			<div>
				<div class="text-2xl font-semibold">{summary.assignmentCount}</div>
				<div class="text-xs text-muted-foreground">งานคุมห้องสอบ</div>
			</div>
		</Card.Content>
	</Card.Root>
	<Card.Root class="gap-0 py-0">
		<Card.Content class="flex items-center gap-3 p-4">
			<div class="rounded-lg bg-primary/10 p-2 text-primary">
				<Clock3 class="size-5" aria-hidden="true" />
			</div>
			<div>
				<div class="text-lg font-semibold">{formatStaffExamMinutes(summary.totalMinutes)}</div>
				<div class="text-xs text-muted-foreground">เวลาคุมสอบจริง</div>
			</div>
		</Card.Content>
	</Card.Root>
</div>

{#if summary.items.length === 0}
	<PageState
		title="ยังไม่มีงานคุมสอบของฉัน"
		description="ไม่พบชื่อของคุณในกรรมการคุมสอบของรอบนี้"
	/>
{:else}
	<div class="space-y-6">
		{#if upcomingItems.length > 0}
			<section class="space-y-3" aria-labelledby="upcoming-invigilation-heading">
				<div>
					<h3 id="upcoming-invigilation-heading" class="font-semibold">งานคุมสอบที่กำลังมาถึง</h3>
					<p class="text-sm text-muted-foreground">เรียงตามวันและเวลาที่ต้องปฏิบัติหน้าที่</p>
				</div>
				<div class="grid gap-3 lg:grid-cols-2">
					{#each upcomingItems as item (item.assignmentId)}
						<Card.Root class="border-primary/20">
							<Card.Header class="gap-2">
								<div class="flex flex-wrap items-start justify-between gap-2">
									<div>
										<Card.Title class="text-base">{formatStaffExamDate(item.examDate)}</Card.Title>
										<Card.Description>
											{item.dayLabel ?? 'วันสอบ'} · {timeBounds(item)}
										</Card.Description>
									</div>
									<Badge>กำลังมาถึง</Badge>
								</div>
							</Card.Header>
							<Card.Content class="grid gap-2 text-sm">
								<div>
									<span class="text-muted-foreground">ชั้นเรียน:</span>
									{item.homeroomName}
								</div>
								<div>
									<span class="text-muted-foreground">ห้องสอบ:</span>
									{roomLabel(item)}
								</div>
								<div>
									<span class="text-muted-foreground">วิชา:</span>
									{subjectNames(item)}
								</div>
								<div>
									<span class="text-muted-foreground">เวลาคุมจริง:</span>
									{formatStaffExamMinutes(item.sessionMinutes)}
								</div>
							</Card.Content>
						</Card.Root>
					{/each}
				</div>
			</section>
		{/if}

		{#if completedItems.length > 0}
			<section class="space-y-3" aria-labelledby="completed-invigilation-heading">
				<div>
					<h3
						id="completed-invigilation-heading"
						class="flex items-center gap-2 font-semibold text-muted-foreground"
					>
						<CheckCircle2 class="size-4" aria-hidden="true" />
						งานที่เสร็จแล้ว
					</h3>
				</div>
				<div class="grid gap-3 opacity-75 lg:grid-cols-2">
					{#each completedItems as item (item.assignmentId)}
						<Card.Root>
							<Card.Header class="gap-1">
								<Card.Title class="text-base">{formatStaffExamDate(item.examDate)}</Card.Title>
								<Card.Description>{timeBounds(item)} · {item.homeroomName}</Card.Description>
							</Card.Header>
							<Card.Content class="grid gap-2 text-sm">
								<div>{roomLabel(item)}</div>
								<div>{subjectNames(item)}</div>
								<div>{formatStaffExamMinutes(item.sessionMinutes)}</div>
							</Card.Content>
						</Card.Root>
					{/each}
				</div>
			</section>
		{/if}
	</div>
{/if}
