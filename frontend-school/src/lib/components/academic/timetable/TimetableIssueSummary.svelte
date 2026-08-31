<script lang="ts">
	import type { WholeSchoolTimetableIssue } from '$lib/api/timetable';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { AlertTriangle, ArrowUpRight, CircleAlert, LocateFixed } from 'lucide-svelte';
	import { SvelteMap } from 'svelte/reactivity';

	let {
		issues,
		homeroomForIssue,
		onFocusIssue,
		onOpenHomeroom,
		onOpenTeacher
	}: {
		issues: WholeSchoolTimetableIssue[];
		homeroomForIssue: (issue: WholeSchoolTimetableIssue) => string | null;
		onFocusIssue: (issue: WholeSchoolTimetableIssue) => void;
		onOpenHomeroom: (homeroomId: string, periodId: string | null) => void;
		onOpenTeacher: (teacherId: string, periodId: string | null) => void;
	} = $props();

	const labels: Record<WholeSchoolTimetableIssue['kind'], string> = {
		homeroom_conflict: 'ห้องเรียนชนกัน',
		instructor_conflict: 'ครูสอนชนกัน',
		room_conflict: 'ห้องเรียนเฉพาะชนกัน',
		unscheduled_demand: 'คาบยังจัดไม่ครบ',
		over_scheduled_demand: 'คาบเกินเป้าหมาย',
		missing_instructor: 'ยังไม่ระบุครู',
		missing_room: 'ยังไม่ระบุห้อง',
		unresolved_teacher_handoff: 'ครูผู้สอนยังไม่ตรงกับวันที่เริ่มใช้'
	};
	const groupedIssues = $derived.by(() => {
		const groups = new SvelteMap<WholeSchoolTimetableIssue['kind'], WholeSchoolTimetableIssue[]>();
		for (const issue of issues) {
			const current = groups.get(issue.kind) ?? [];
			current.push(issue);
			groups.set(issue.kind, current);
		}
		return [...groups.entries()];
	});
</script>

<aside class="overflow-hidden rounded-xl border bg-background" aria-label="จุดที่ต้องตรวจสอบ">
	<div class="flex items-start justify-between gap-3 border-b bg-muted/20 px-4 py-3">
		<div>
			<h2 class="font-semibold">จุดที่ต้องตรวจสอบ</h2>
			<p class="text-xs text-muted-foreground">กดดูตำแหน่ง หรือเปิดมุมมองที่แก้ไขได้</p>
		</div>
		<Badge
			variant={issues.some((issue) => issue.severity === 'blocking') ? 'destructive' : 'secondary'}
		>
			{issues.length} จุด
		</Badge>
	</div>

	{#if issues.length === 0}
		<div class="flex flex-col items-center gap-2 px-5 py-10 text-center">
			<CircleAlert class="size-7 text-emerald-600" />
			<p class="text-sm font-medium">ไม่พบจุดผิดปกติในวันนี้</p>
			<p class="text-xs text-muted-foreground">ข้อมูลสรุปนี้อ้างอิงรุ่นตารางและวันที่เลือก</p>
		</div>
	{:else}
		<div class="max-h-[42rem] space-y-4 overflow-y-auto p-3">
			{#each groupedIssues as [kind, group] (kind)}
				<section class="space-y-2" aria-label={labels[kind]}>
					<div class="flex items-center gap-2 px-1">
						<AlertTriangle class="size-3.5 text-amber-600" />
						<h3 class="text-xs font-semibold">{labels[kind]}</h3>
						<Badge variant="outline" class="ms-auto">{group.length}</Badge>
					</div>
					{#each group as issue, issueIndex (`${issue.kind}:${issue.learningGroupId ?? ''}:${issue.bellSchedulePeriodId ?? ''}:${issueIndex}`)}
						{@const homeroomId = issue.homeroomIds[0] ?? homeroomForIssue(issue)}
						<article
							class={[
								'rounded-lg border p-3',
								issue.severity === 'blocking'
									? 'border-destructive/25 bg-destructive/5'
									: 'bg-muted/15'
							]}
						>
							<p class="text-xs leading-5">{issue.message}</p>
							<div class="mt-2 flex flex-wrap gap-1.5">
								{#if issue.bellSchedulePeriodId || homeroomId}
									<Button
										size="sm"
										variant="ghost"
										class="h-7 px-2 text-xs"
										onclick={() => onFocusIssue(issue)}
									>
										<LocateFixed class="size-3" /> ดูช่อง
									</Button>
								{/if}
								{#if homeroomId}
									<Button
										size="sm"
										variant="outline"
										class="h-7 px-2 text-xs"
										onclick={() => onOpenHomeroom(homeroomId, issue.bellSchedulePeriodId)}
									>
										แก้ในตารางห้อง <ArrowUpRight class="size-3" />
									</Button>
								{/if}
								{#if issue.instructorIds[0]}
									<Button
										size="sm"
										variant="outline"
										class="h-7 px-2 text-xs"
										onclick={() =>
											onOpenTeacher(issue.instructorIds[0], issue.bellSchedulePeriodId)}
									>
										เปิดตารางครู <ArrowUpRight class="size-3" />
									</Button>
								{/if}
							</div>
						</article>
					{/each}
				</section>
			{/each}
		</div>
	{/if}
</aside>
