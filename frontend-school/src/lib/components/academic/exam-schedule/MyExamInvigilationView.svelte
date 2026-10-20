<script lang="ts">
	import { PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import {
		formatStaffExamDate,
		formatStaffExamMinutes,
		formatStaffExamTime,
		type MyExamInvigilationItem,
		type MyExamInvigilationSummary
	} from '#lib/utils/staff-exam-schedule-view.js';

	interface Props {
		summary: MyExamInvigilationSummary;
	}

	let { summary }: Props = $props();

	function roomLabel(item: MyExamInvigilationItem): string {
		return [item.buildingName, item.roomName].filter(Boolean).join(' · ') || '-';
	}

	function timeBounds(item: MyExamInvigilationItem): string {
		if (!item.earliestStartsAt && !item.latestEndsAt) return '-';
		return `${formatStaffExamTime(item.earliestStartsAt ?? '')}–${formatStaffExamTime(
			item.latestEndsAt ?? ''
		)}`;
	}
</script>

<div class="space-y-4">
	<dl
		class="flex flex-wrap gap-x-6 gap-y-2 rounded-xl border bg-card p-3 text-sm sm:p-4"
		aria-label="สรุปงานคุมสอบของฉันในรอบนี้"
	>
		<div>
			<dt class="text-xs text-muted-foreground">วันที่ได้รับมอบหมาย</dt>
			<dd class="mt-1 font-semibold tabular-nums">{summary.assignedDayCount} วัน</dd>
		</div>
		<div>
			<dt class="text-xs text-muted-foreground">งานคุมห้องสอบ</dt>
			<dd class="mt-1 font-semibold tabular-nums">{summary.assignmentCount} งาน</dd>
		</div>
		<div>
			<dt class="text-xs text-muted-foreground">เวลาคุมสอบจริง</dt>
			<dd class="mt-1 font-semibold tabular-nums">
				{formatStaffExamMinutes(summary.totalMinutes)}
			</dd>
		</div>
	</dl>
	{#if summary.items.length === 0}
		<PageState
			title="ยังไม่มีงานคุมสอบของฉัน"
			description="ไม่พบชื่อของคุณในกรรมการคุมสอบของรอบนี้"
		/>
	{:else}
		<ul class="divide-y rounded-xl border bg-card" aria-label="งานคุมสอบของฉัน">
			{#each summary.items as item (item.assignmentId)}
				<li class="space-y-2 p-3 sm:p-4">
					<div class="flex flex-wrap items-center justify-between gap-2">
						<h2 class="text-sm font-semibold">
							{formatStaffExamDate(item.examDate)}{item.dayLabel ? ` · ${item.dayLabel}` : ''}
						</h2>
						<Badge variant={item.status === 'upcoming' ? 'default' : 'secondary'}
							>{item.status === 'upcoming' ? 'กำลังมาถึง' : 'เสร็จแล้ว'}</Badge
						>
					</div>
					<p class="text-sm">{item.homeroomName} · {roomLabel(item)}</p>
					<p class="text-sm text-muted-foreground">
						<span class="tabular-nums">{timeBounds(item)}</span> · เวลาคุมจริง {formatStaffExamMinutes(
							item.sessionMinutes
						)}
					</p>
					<ul class="space-y-1 text-sm" aria-label="รายวิชาที่คุมสอบ">
						{#each item.sessions as session (session.sessionId)}
							<li class="flex flex-wrap gap-x-2 gap-y-0.5">
								<span class="tabular-nums text-muted-foreground"
									>{formatStaffExamTime(session.startsAt)}–{formatStaffExamTime(
										session.endsAt
									)}</span
								><span
									>{session.subjectName}
									<span class="text-muted-foreground"
										>({session.subjectCode}) · {session.assessmentPhaseName}</span
									></span
								>
							</li>
						{/each}
					</ul>
				</li>
			{/each}
		</ul>
	{/if}
</div>
