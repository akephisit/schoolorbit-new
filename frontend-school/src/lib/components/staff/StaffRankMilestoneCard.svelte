<script lang="ts">
	import { CalendarClock, Info } from '@lucide/svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { buttonVariants } from '#lib/components/ui/button/index.js';
	import { formatCareerDate } from '#lib/forms/staff-career.js';
	import { ACADEMIC_RANK_LABELS } from '#lib/forms/staff-personnel.js';
	import type { RankMilestone } from '#lib/api/personnel.js';
	let { milestone }: { milestone: RankMilestone } = $props();
	import { RANK_MILESTONE_LABELS as labels } from '#lib/forms/rank-milestones.js';
	const nextLabel = $derived(milestone.nextRank ? ACADEMIC_RANK_LABELS[milestone.nextRank] : '—');
</script>

<section
	aria-label="กำหนดเวลาวิทยฐานะ"
	class="space-y-4 rounded-xl border bg-card p-5"
	data-testid="rank-milestone-card"
>
	<div class="flex flex-wrap items-start justify-between gap-3">
		<div class="flex items-center gap-3">
			<span
				class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary"
				><CalendarClock class="size-5" aria-hidden="true" /></span
			>
			<div>
				<h3 class="font-semibold">กำหนดเวลาวิทยฐานะ</h3>
				<p class="text-sm text-muted-foreground">วางแผนจากวันที่มีผลในประวัติปัจจุบัน</p>
			</div>
		</div>
		<Badge variant="secondary" class="whitespace-normal leading-relaxed"
			>{labels[milestone.status]}</Badge
		>
	</div>
	{#if milestone.ordinaryDate}
		<div class="grid gap-4 sm:grid-cols-2">
			<div class="space-y-2 rounded-lg bg-primary/5 p-4">
				<p class="text-sm text-muted-foreground">
					ครบระยะเวลาตามเกณฑ์ปกติ {milestone.criteria?.ordinaryYears} ปี
				</p>
				<p class="text-xl font-semibold leading-relaxed">
					{formatCareerDate(milestone.ordinaryDate)}
				</p>
				<p class="text-sm">เป้าหมาย: {nextLabel}</p>
				{#if milestone.daysUntilOrdinaryDate !== null && milestone.daysUntilOrdinaryDate > 0}<p
						class="text-sm text-muted-foreground"
					>
						อีก {milestone.daysUntilOrdinaryDate.toLocaleString('th-TH')} วัน
					</p>{/if}
			</div>
			<div class="space-y-2 rounded-lg border border-dashed p-4">
				<p class="text-sm text-muted-foreground">
					กรณีเข้าเงื่อนไขลดระยะเวลา {milestone.criteria?.conditionalReducedYears} ปี
				</p>
				<p class="text-xl font-semibold leading-relaxed">
					{formatCareerDate(milestone.conditionalReducedDate)}
				</p>
				<Badge variant="outline" class="whitespace-normal leading-relaxed"
					>ต้องตรวจหลักฐานสิทธิลดระยะเวลา</Badge
				>
				<p class="text-xs leading-relaxed text-muted-foreground">
					ไม่ได้ยืนยันสิทธิ์จากวุฒิการศึกษาหรือวันที่เพียงอย่างเดียว
				</p>
			</div>
		</div>
		<p class="text-xs leading-relaxed text-muted-foreground">
			นับจากวันที่มีผลล่าสุดของข้อมูลที่ใช้ร่วมกัน: {formatCareerDate(milestone.recordedStartDate)}
		</p>
	{:else}
		<ul class="space-y-1 text-sm leading-relaxed text-muted-foreground">
			{#each milestone.reasonLabels as reason (reason)}<li>{reason}</li>{/each}
		</ul>
	{/if}
	<div class="flex items-start gap-2 rounded-lg bg-muted/50 p-3 text-xs leading-relaxed">
		<Info class="mt-0.5 size-4 shrink-0" aria-hidden="true" />
		<p>
			วันที่นี้เป็นกำหนดเวลาจากข้อมูลที่บันทึก ยังต้องตรวจการดำรงตำแหน่งต่อเนื่อง ผล PA ภาระงาน
			วินัย และคุณสมบัติตามหลักเกณฑ์ก่อนยื่น รวมถึงกรณีเทียบตำแหน่ง พื้นที่พิเศษ และเส้นทางอื่น
		</p>
	</div>
	{#if milestone.criteria}
		<details class="text-xs text-muted-foreground">
			<summary class="cursor-pointer py-1 font-medium"
				>หลักเกณฑ์ที่ใช้อ้างอิง · ตรวจข้อมูล ณ {formatCareerDate(milestone.asOf)}</summary
			>
			<div class="space-y-2 pt-3">
				<p>
					ฉบับ {milestone.criteria.version} · ตรวจเกณฑ์เมื่อ {formatCareerDate(
						milestone.criteria.reviewedOn
					)} · วันในอนาคตอาจเปลี่ยนตามหลักเกณฑ์ใหม่
				</p>
				<div class="flex flex-wrap gap-2">
					{#each milestone.criteria.sources as source (source.url)}<a
							href={source.url}
							target="_blank"
							rel="external noopener noreferrer"
							class={buttonVariants({
								variant: 'outline',
								size: 'sm',
								class: 'h-auto whitespace-normal text-left no-underline'
							})}>{source.title}</a
						>{/each}
				</div>
			</div>
		</details>
	{/if}
</section>
