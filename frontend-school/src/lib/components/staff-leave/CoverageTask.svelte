<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { confirmLeaveCoverage, type LeaveCoverageTask } from '#lib/api/staff-leave.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import { coverageConsentLabels, coverageReturnCaption } from '#lib/staff-leave/coverage.js';
	import { leaveDate } from '#lib/staff-leave/presentation.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	let {
		task,
		onpatched,
		onrefresh
	}: {
		task: LeaveCoverageTask;
		onpatched: (value: LeaveCoverageTask) => void;
		onrefresh: () => void;
	} = $props();
	let selected = $state<string[]>(
			untrack(() =>
				task.assignments.flatMap((lesson) =>
					lesson.assignment?.consent === 'pending' ? [lesson.assignment.id] : []
				)
			)
		),
		pending = $state(''),
		error = $state('');
	const allowed = $derived(
		$can.has(PERMISSIONS.STAFF_LEAVE_READ_ASSIGNED) &&
			$can.has(PERMISSIONS.STAFF_LEAVE_UPDATE_ASSIGNED)
	);
	const intent = new LeaveMutationIntent();
	let disposed = false;
	onDestroy(() => {
		disposed = true;
	});
	async function decide(decision: 'confirmed' | 'declined') {
		if (!allowed || pending || !selected.length) return;
		pending = decision;
		error = '';
		const basis = { expectedRevision: task.planRevision, assignmentIds: selected, decision };
		try {
			const result = await confirmLeaveCoverage(task.requestId, {
				requestId: intent.for(basis),
				...basis
			});
			if (!disposed) {
				selected = [];
				onpatched(result);
			}
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ตอบรับไม่สำเร็จ';
		} finally {
			if (!disposed) pending = '';
		}
	}
</script>

<section class="space-y-4 rounded-xl border bg-card p-4" data-testid="leave-assigned-task">
	<div>
		<h2 class="font-semibold">คาบสอนของ {task.ownerName}</h2>
		<p class="text-sm text-muted-foreground">แสดงเฉพาะคาบที่มอบหมายให้คุณ</p>
	</div>
	<ul class="divide-y">
		{#each task.assignments as lesson (lesson.lessonKey)}
			{#if lesson.assignment}<li class="space-y-2 py-3">
					<div class="flex items-start gap-3">
						{#if allowed}<Checkbox
								id={`task-${lesson.assignment.id}`}
								aria-label={`เลือก ${lesson.date} ${lesson.periodName}`}
								checked={selected.includes(lesson.assignment.id)}
								disabled={!!pending}
								onCheckedChange={(checked) =>
									(selected = checked
										? [...selected, lesson.assignment!.id]
										: selected.filter((value) => value !== lesson.assignment!.id))}
							/>{/if}
						<div class="min-w-0 flex-1">
							<Label for={`task-${lesson.assignment.id}`}
								>{leaveDate(lesson.date)} · {lesson.periodName} · {lesson.startTime.slice(
									0,
									5
								)}–{lesson.endTime.slice(0, 5)}</Label
							>
							<p class="break-words text-sm">
								{lesson.offeringName ?? 'กิจกรรมตามตาราง'} · {lesson.homeroomNames.join(
									', '
								)}{lesson.roomName ? ` · ${lesson.roomName}` : ''}
							</p>
						</div>
						<Badge variant="outline">{coverageConsentLabels[lesson.assignment.consent]}</Badge>
					</div>
					<p class="text-sm">
						{lesson.assignment.mode === 'reciprocal' ? 'แลกคาบสอน' : 'สอนแทน'} · สอนจริง: {lesson
							.assignment.taughtOfferingName ??
							lesson.assignment.taughtActivity ??
							'ยังไม่ระบุ'}
					</p>
					{#if lesson.assignment.taughtOfferingName && lesson.assignment.taughtActivity}<p
							class="text-sm"
						>
							{lesson.assignment.taughtActivity}
						</p>{/if}
					{#if lesson.assignment.returnDate}<p class="text-sm text-muted-foreground">
							ผู้ขอจะสอน{coverageReturnCaption(
								lesson,
								lesson.assignment,
								leaveDate(lesson.assignment.returnDate)
							)}
						</p>{/if}
				</li>{/if}{/each}
	</ul>
	{#if error}<PageState
			variant="error"
			title="ตอบรับคาบสอนไม่สำเร็จ"
			description={error}
			actionLabel="ตรวจสอบข้อมูลล่าสุด"
			onaction={onrefresh}
		/>{/if}
	{#if allowed}<div class="flex flex-wrap items-center justify-end gap-2">
			<p class="mr-auto text-sm text-muted-foreground">เลือก {selected.length} คาบ</p>
			<LoadingButton
				variant="outline"
				loading={pending === 'declined'}
				disabled={!!pending || !selected.length}
				onclick={() => decide('declined')}>ปฏิเสธคาบที่เลือก</LoadingButton
			><LoadingButton
				loading={pending === 'confirmed'}
				disabled={!!pending || !selected.length}
				onclick={() => decide('confirmed')}>ตอบรับคาบที่เลือก</LoadingButton
			>
		</div>{/if}
</section>
