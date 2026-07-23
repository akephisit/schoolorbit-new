<script lang="ts">
	import type {
		ExamDayDetail,
		ExamRoundStatus,
		ExamScheduleItem,
		ExamScheduleReadiness,
		ExamSession
	} from '$lib/api/examSchedule';
	import { Badge } from '$lib/components/ui/badge';
	import { AlertCircle, CheckCircle2, CircleDashed } from 'lucide-svelte';

	let {
		status = 'draft',
		readiness,
		days = [],
		unscheduledItems = [],
		scheduledSessions = []
	}: {
		status?: ExamRoundStatus;
		readiness: ExamScheduleReadiness;
		days: ExamDayDetail[];
		unscheduledItems: ExamScheduleItem[];
		scheduledSessions: ExamSession[];
	} = $props();

	const assignmentCount = $derived(
		days.reduce((total, day) => total + day.roomAssignments.length, 0)
	);
	const blockedWindowCount = $derived(
		days.reduce((total, day) => total + day.blockedWindows.length, 0)
	);
	const totalItems = $derived(unscheduledItems.length + scheduledSessions.length);
</script>

<section class="rounded-md border bg-background">
	<div class="border-b px-4 py-4">
		<div class="flex items-center justify-between gap-3">
			<h2 class="font-semibold">ความพร้อม</h2>
			<Badge variant={readiness.canPublish ? 'default' : 'secondary'}>
				{readiness.canPublish ? 'พร้อมเผยแพร่' : 'ยังไม่พร้อม'}
			</Badge>
		</div>
		<p class="mt-1 text-sm text-muted-foreground">
			{status === 'published' ? 'เผยแพร่แล้ว' : 'ฉบับร่าง'}
		</p>
	</div>

	<div class="space-y-4 p-4">
		<div class="grid grid-cols-2 gap-2 text-sm">
			<div class="rounded-md border bg-muted/20 p-3">
				<div class="text-muted-foreground">วันสอบ</div>
				<div class="mt-1 text-xl font-semibold">{days.length}</div>
			</div>
			<div class="rounded-md border bg-muted/20 p-3">
				<div class="text-muted-foreground">ห้องสอบ</div>
				<div class="mt-1 text-xl font-semibold">{assignmentCount}</div>
			</div>
			<div class="rounded-md border bg-muted/20 p-3">
				<div class="text-muted-foreground">รายการสอบ</div>
				<div class="mt-1 text-xl font-semibold">{totalItems}</div>
			</div>
			<div class="rounded-md border bg-muted/20 p-3">
				<div class="text-muted-foreground">ช่วงปิด</div>
				<div class="mt-1 text-xl font-semibold">{blockedWindowCount}</div>
			</div>
		</div>

		<div class="space-y-2">
			<div class="flex items-center gap-2 text-sm font-medium">
				{#if readiness.canPublish}
					<CheckCircle2 class="h-4 w-4 text-emerald-600" />
				{:else}
					<AlertCircle class="h-4 w-4 text-amber-600" />
				{/if}
				ตัวตรวจสอบ
			</div>

			{#if readiness.blockers.length === 0}
				<div
					class="flex items-start gap-2 rounded-md border border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-900"
				>
					<CheckCircle2 class="mt-0.5 h-4 w-4 shrink-0" />
					<span>ไม่มีรายการติดขัด</span>
				</div>
			{:else}
				<div class="space-y-2">
					{#each readiness.blockers as blocker, index (`${index}-${blocker}`)}
						<div class="flex items-start gap-2 rounded-md border p-3 text-sm">
							<CircleDashed class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
							<span>{blocker}</span>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</section>
