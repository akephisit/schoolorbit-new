<script lang="ts">
	import type { BudgetActivity } from '#lib/api/budget.js';
	import { activityMonths, activitiesInMonth, monthLabel } from '#lib/budget/schedule.js';
	import { dateLabel } from '#lib/budget/presentation.js';
	import Choice from './Choice.svelte';
	let { activities }: { activities: BudgetActivity[] } = $props();
	const controlId = $props.id();
	let selected = $state('');
	const months = $derived(activityMonths(activities));
	const month = $derived(months.includes(selected) ? selected : (months[0] ?? ''));
	const visible = $derived(month ? activitiesInMonth(activities, month) : []);
	const undated = $derived(
		activities.filter((activity) => !activity.startDate || !activity.endDate).length
	);
</script>

<section class="space-y-3 rounded-xl border bg-card p-4" aria-label="ช่วงเวลากิจกรรมรายเดือน">
	<div class="flex flex-wrap items-end justify-between gap-3">
		<div>
			<h2 class="font-semibold">ช่วงเวลากิจกรรมรายเดือน</h2>
			<p class="text-sm text-muted-foreground">แสดงกิจกรรมที่มีช่วงเวลาทับซ้อนกับเดือนที่เลือก</p>
		</div>
		{#if months.length}<div class="w-full sm:w-56">
				<Choice
					id={`${controlId}-month`}
					label="เดือน"
					value={month}
					options={months.map((value) => ({ value, label: monthLabel(value) }))}
					onchange={(value) => (selected = value)}
				/>
			</div>{/if}
	</div>
	{#if visible.length}<ul class="divide-y">
			{#each visible as activity (activity.id)}<li
					class="flex flex-wrap justify-between gap-2 py-2 text-sm"
				>
					<span class="font-medium">{activity.title}</span><span class="text-muted-foreground"
						>{dateLabel(activity.startDate!)} – {dateLabel(activity.endDate!)}</span
					>
				</li>{/each}
		</ul>{:else}<p class="text-sm text-muted-foreground">
			ยังไม่มีช่วงเวลากิจกรรมที่บันทึกไว้
		</p>{/if}{#if undated}<p class="text-sm text-muted-foreground">
			{undated} กิจกรรมยังไม่บันทึกวันที่ โปรดระบุวันที่เมื่อแก้ไขแผน
		</p>{/if}
</section>
