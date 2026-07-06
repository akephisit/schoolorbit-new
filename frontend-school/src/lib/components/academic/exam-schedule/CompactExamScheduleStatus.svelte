<script lang="ts">
	import type {
		ExamDayDetail,
		ExamRoundStatus,
		ExamScheduleItem,
		ExamScheduleReadiness,
		ExamSession
	} from '$lib/api/examSchedule';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Sheet from '$lib/components/ui/sheet';
	import { AlertTriangle, CheckCircle2, Eye } from 'lucide-svelte';

	let {
		status = 'draft',
		readiness,
		days = [],
		unscheduledItems = [],
		scheduledSessions = [],
		invigilatorAssignedCount = 0,
		invigilatorAssignmentCount = 0
	}: {
		status?: ExamRoundStatus;
		readiness: ExamScheduleReadiness;
		days?: ExamDayDetail[];
		unscheduledItems?: ExamScheduleItem[];
		scheduledSessions?: ExamSession[];
		invigilatorAssignedCount?: number;
		invigilatorAssignmentCount?: number;
	} = $props();

	const statusLabel = $derived(status === 'published' ? 'เผยแพร่แล้ว' : 'ฉบับร่าง');
	const statusVariant = $derived(status === 'published' ? 'default' : 'secondary');
	const totalItems = $derived(unscheduledItems.length + scheduledSessions.length);
	const roomAssignmentCount = $derived(
		days.reduce((total, day) => total + day.roomAssignments.length, 0)
	);
</script>

<section class="rounded-md border bg-background px-4 py-3">
	<div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
		<div class="flex min-w-0 flex-wrap items-center gap-2">
			<Badge variant={statusVariant}>{statusLabel}</Badge>
			<Badge
				variant="outline"
				class={readiness.canPublish
					? 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-300'
					: 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-300'}
			>
				{#if readiness.canPublish}
					<CheckCircle2 class="h-3 w-3" />
					พร้อมเผยแพร่
				{:else}
					<AlertTriangle class="h-3 w-3" />
					ยังไม่พร้อม
				{/if}
			</Badge>
			<Badge variant="outline">ยังไม่จัด {unscheduledItems.length}/{totalItems}</Badge>
			<Badge variant="outline">ห้องสอบ {roomAssignmentCount}</Badge>
			<Badge variant="outline">กรรมการ {invigilatorAssignedCount}/{invigilatorAssignmentCount}</Badge>
		</div>

		<Sheet.Root>
			<Sheet.Trigger>
				{#snippet child({ props })}
					<Button variant="outline" size="sm" {...props}>
						<Eye class="h-4 w-4" />
						ดูความพร้อม
					</Button>
				{/snippet}
			</Sheet.Trigger>
			<Sheet.Content side="right" class="overflow-hidden sm:max-w-lg">
				<Sheet.Header>
					<Sheet.Title>ความพร้อมก่อนเผยแพร่</Sheet.Title>
					<Sheet.Description>รายการตรวจสอบของรอบตารางสอบนี้</Sheet.Description>
				</Sheet.Header>

				<div class="min-h-0 flex-1 overflow-y-auto pr-1">
					{#if readiness.blockers.length === 0}
						<div
							class="flex items-start gap-3 rounded-md border border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-900 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-200"
						>
							<CheckCircle2 class="mt-0.5 h-4 w-4 shrink-0" />
							<div class="space-y-1">
								<p class="font-medium">ไม่มีรายการติดขัด</p>
								<p class="text-emerald-800 dark:text-emerald-300">
									รอบตารางสอบนี้ผ่านรายการตรวจสอบก่อนเผยแพร่แล้ว
								</p>
							</div>
						</div>
					{:else}
						<div class="space-y-2">
							{#each readiness.blockers as blocker, index (`${index}-${blocker}`)}
								<div class="flex items-start gap-3 rounded-md border bg-background p-3 text-sm">
									<AlertTriangle class="mt-0.5 h-4 w-4 shrink-0 text-amber-600" />
									<span>{blocker}</span>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</Sheet.Content>
		</Sheet.Root>
	</div>
</section>
