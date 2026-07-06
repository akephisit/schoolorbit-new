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
	import { AlertTriangle, CheckCircle2 } from 'lucide-svelte';

	let {
		status = 'draft',
		readiness,
		days = [],
		unscheduledItems = [],
		scheduledSessions = [],
		invigilatorAssignedCount,
		invigilatorAssignmentCount
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
	const invigilatorAssignmentFallback = $derived(
		days.reduce((total, day) => total + day.roomAssignments.length, 0)
	);
	const invigilatorAssignedFallback = $derived(
		days.reduce(
			(total, day) =>
				total +
				day.roomAssignments.filter((assignment) => assignment.invigilators.length > 0).length,
			0
		)
	);
	const roomAssignmentCount = $derived(invigilatorAssignmentFallback);
	const displayedInvigilatorAssignedCount = $derived(
		invigilatorAssignedCount ?? invigilatorAssignedFallback
	);
	const displayedInvigilatorAssignmentCount = $derived(
		invigilatorAssignmentCount ?? invigilatorAssignmentFallback
	);
</script>

<Sheet.Root>
	<Sheet.Trigger>
		{#snippet child({ props })}
			<Button
				variant="outline"
				size="sm"
				class={readiness.canPublish
					? 'border-emerald-200 bg-emerald-50 text-emerald-700 hover:bg-emerald-100 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-300 dark:hover:bg-emerald-900'
					: 'border-amber-200 bg-amber-50 text-amber-700 hover:bg-amber-100 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-300 dark:hover:bg-amber-900'}
				title="ดูความพร้อม"
				{...props}
			>
				{#if readiness.canPublish}
					<CheckCircle2 class="h-4 w-4" />
					พร้อม
				{:else}
					<AlertTriangle class="h-4 w-4" />
					ยังไม่พร้อม {readiness.blockers.length}
				{/if}
			</Button>
		{/snippet}
	</Sheet.Trigger>
	<Sheet.Content side="right" class="overflow-hidden sm:max-w-lg">
		<Sheet.Header>
			<Sheet.Title>ความพร้อมก่อนเผยแพร่</Sheet.Title>
			<Sheet.Description>รายการตรวจสอบของรอบตารางสอบนี้</Sheet.Description>
		</Sheet.Header>

		<div class="min-h-0 flex-1 space-y-4 overflow-y-auto pr-1">
			<div class="grid grid-cols-2 gap-2 text-sm">
				<div class="rounded-md border bg-background p-3">
					<p class="text-xs text-muted-foreground">สถานะรอบ</p>
					<Badge class="mt-2" variant={statusVariant}>{statusLabel}</Badge>
				</div>
				<div class="rounded-md border bg-background p-3">
					<p class="text-xs text-muted-foreground">รายการสอบ</p>
					<p class="mt-2 font-medium">ยังไม่จัด {unscheduledItems.length}/{totalItems}</p>
				</div>
				<div class="rounded-md border bg-background p-3">
					<p class="text-xs text-muted-foreground">ห้องสอบ</p>
					<p class="mt-2 font-medium">{roomAssignmentCount} ห้อง</p>
				</div>
				<div class="rounded-md border bg-background p-3">
					<p class="text-xs text-muted-foreground">กรรมการ</p>
					<p class="mt-2 font-medium">
						{displayedInvigilatorAssignedCount}/{displayedInvigilatorAssignmentCount}
					</p>
				</div>
			</div>

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
