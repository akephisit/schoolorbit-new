<script lang="ts">
	import type { SupervisionObservation } from '#lib/api/supervision.js';
	import { savedPeriodSourceLabel } from '#lib/supervision/supervision-presentation.js';
	import { savedPeriodLabel } from '#lib/supervision/supervision-schedule.js';
	let { observation }: { observation: SupervisionObservation } = $props();
	const periods = $derived([...observation.periods].sort((a, b) => a.position - b.position));
	function dateLabel(value: string) {
		return new Date(value).toLocaleString('th-TH', {
			timeZone: 'Asia/Bangkok',
			dateStyle: 'medium',
			timeStyle: 'short'
		});
	}
</script>

<ol class="divide-y rounded-lg border bg-muted/15" data-testid="supervision-saved-periods">
	{#each periods as period (period.id)}
		<li class="space-y-1 p-3 text-sm wrap-anywhere sm:px-4">
			<p class="font-semibold">
				{savedPeriodLabel(period) ?? 'ไม่ระบุคาบ'} · {period.lessonSnapshot.subjectName ??
					period.manualLesson?.subjectName ??
					'ไม่ระบุวิชา'}
			</p>
			<p class="text-muted-foreground">
				{period.lessonSnapshot.classroomLabel ??
					period.manualLesson?.classroomLabel ??
					'ไม่ระบุชั้น/ห้อง'} · {period.lessonSnapshot.roomLabel ??
					period.manualLesson?.roomLabel ??
					'ไม่ระบุห้องเรียน'}
			</p>
			<p class="text-xs tabular-nums text-muted-foreground">
				{dateLabel(period.observedAt)}{#if period.endsAt}
					– {new Date(period.endsAt).toLocaleTimeString('th-TH', {
						timeZone: 'Asia/Bangkok',
						hour: '2-digit',
						minute: '2-digit'
					})}{/if} · {savedPeriodSourceLabel(period)}
			</p>
			{#if period.manualLesson?.reason}<p class="mt-2 whitespace-pre-wrap text-muted-foreground">
					หมายเหตุเดิม: {period.manualLesson.reason}
				</p>{/if}
		</li>
	{/each}
</ol>
