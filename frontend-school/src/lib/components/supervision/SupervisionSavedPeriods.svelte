<script lang="ts">
	import type { SupervisionObservation } from '#lib/api/supervision.js';
	import { savedPeriodSourceLabel } from '#lib/supervision/supervision-presentation.js';
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

<div class="space-y-2" data-testid="supervision-saved-periods">
	{#each periods as period (period.id)}
		<div class="rounded-lg border p-3 text-sm">
			<p class="font-medium">
				{period.lessonSnapshot.periodLabel ??
					period.manualLesson?.periodLabel ??
					`คาบ ${period.position}`} · {period.lessonSnapshot.subjectName ??
					period.manualLesson?.subjectName ??
					'ไม่ระบุวิชา'}
			</p>
			<p>
				{period.lessonSnapshot.classroomLabel ??
					period.manualLesson?.classroomLabel ??
					'ไม่ระบุชั้น/ห้อง'} · {period.lessonSnapshot.roomLabel ??
					period.manualLesson?.roomLabel ??
					'ไม่ระบุห้องเรียน'}
			</p>
			<p class="mt-1 text-xs text-muted-foreground">
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
		</div>
	{/each}
</div>
