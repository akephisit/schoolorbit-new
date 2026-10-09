<script lang="ts">
	import type { PublicSchoolStatistics } from '#lib/api/school.js';
	import { summarizeEducationLevels } from '#lib/school-public/statistics.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { GraduationCap, BookOpen, UsersRound, DoorOpen, ChevronDown } from '@lucide/svelte';
	let { statistics }: { statistics: PublicSchoolStatistics } = $props();
	const number = new Intl.NumberFormat('th-TH');
	let expandedGrades = $state<string[]>([]);
	const educationLevels = $derived(summarizeEducationLevels(statistics));
	const cards = $derived([
		{
			label: 'นักเรียน',
			value: statistics.academicYear ? statistics.students.total : null,
			icon: GraduationCap,
			note: 'ในปีการศึกษาปัจจุบัน'
		},
		{
			label: 'ครู',
			value: statistics.totalTeachers,
			icon: BookOpen,
			note: 'ตามตำแหน่งครูที่ปฏิบัติงานอยู่'
		},
		{
			label: 'บุคลากรรวม',
			value: statistics.totalStaff,
			icon: UsersRound,
			note: 'รวมครู ผู้บริหาร และบุคลากรอื่น'
		},
		{
			label: 'ห้องเรียน',
			value: statistics.academicYear ? statistics.totalHomerooms : null,
			icon: DoorOpen,
			note: 'ห้องประจำชั้นที่เปิดใช้งาน'
		}
	]);
	function gradeName(level: string, year: number) {
		const names: Record<string, string> = {
			kindergarten: 'อนุบาล',
			primary: 'ประถมศึกษาปีที่',
			secondary: 'มัธยมศึกษาปีที่'
		};
		return `${names[level] ?? level} ${year}`;
	}
</script>

<div class="grid grid-cols-2 gap-3 lg:grid-cols-4 sm:gap-4" data-testid="school-statistics">
	{#each cards as card (card.label)}
		<div class="public-surface rounded-2xl border border-border bg-card p-4 sm:p-6">
			<div class="flex items-center justify-between gap-2">
				<p class="text-sm text-muted-foreground">{card.label}</p>
				<card.icon class="size-5 shrink-0 text-primary" />
			</div>
			<p class="mt-4 text-3xl font-semibold tracking-tight sm:text-4xl">
				{card.value === null ? '—' : number.format(card.value)}
				<span class="text-xs font-normal text-muted-foreground"
					>{card.label === 'ห้องเรียน' ? 'ห้อง' : 'คน'}</span
				>
			</p>
			<p class="mt-2 text-xs leading-relaxed text-muted-foreground">{card.note}</p>
		</div>
	{/each}
</div>

<div class="mt-10 flex flex-wrap items-end justify-between gap-4">
	<div>
		<h3 class="text-xl font-medium sm:text-2xl">นักเรียนในแต่ละชั้น</h3>
		<p class="mt-2 text-sm text-muted-foreground">
			{statistics.academicYear?.name ?? 'ยังไม่มีปีการศึกษาที่เปิดใช้งาน'} · กดระดับชั้นเพื่อดูตารางรายห้อง
		</p>
	</div>
	<p class="text-xs text-muted-foreground">
		ข้อมูล ณ {new Intl.DateTimeFormat('th-TH', {
			dateStyle: 'medium',
			timeZone: 'Asia/Bangkok'
		}).format(new Date(statistics.asOf))}
	</p>
</div>

{#if !statistics.academicYear}
	<div class="mt-5">
		<PageState
			title="ยังไม่มีปีการศึกษาที่เปิดใช้งาน"
			description="สถิตินักเรียนและห้องเรียนจะแสดงเมื่อโรงเรียนเปิดใช้งานปีการศึกษา"
		/>
	</div>
{:else if !statistics.grades.length}
	<div class="mt-5"><PageState title="ยังไม่มีข้อมูลนักเรียนและห้องเรียนในปีนี้" /></div>
{:else}
	<div class="mt-5 overflow-x-auto rounded-xl border bg-card" data-testid="school-level-summary">
		<table class="w-full min-w-96 text-sm">
			<caption class="px-4 py-3 text-left font-medium">สรุปนักเรียนแยกตามช่วงชั้น</caption>
			<thead class="bg-muted/50 text-muted-foreground"
				><tr>
					<th scope="col" class="p-3 text-left font-medium">ช่วงชั้น</th>
					<th scope="col" class="p-3 text-right font-medium">ชาย</th>
					<th scope="col" class="p-3 text-right font-medium">หญิง</th>
					<th scope="col" class="p-3 text-right font-medium">อื่น ๆ / ไม่ระบุ</th>
					<th scope="col" class="p-3 text-right font-medium">ทั้งหมด</th>
				</tr></thead
			>
			<tbody class="divide-y divide-border">
				{#each educationLevels as level (level.id)}<tr>
						<th scope="row" class="p-3 text-left font-medium">{level.label}</th>
						<td class="p-3 text-right tabular-nums">{number.format(level.students.male)}</td>
						<td class="p-3 text-right tabular-nums">{number.format(level.students.female)}</td>
						<td class="p-3 text-right tabular-nums"
							>{number.format(level.students.otherOrUnspecified)}</td
						>
						<td class="p-3 text-right font-semibold tabular-nums"
							>{number.format(level.students.total)}</td
						>
					</tr>{/each}
			</tbody>
			<tfoot class="border-t bg-muted/50 font-semibold"
				><tr>
					<th scope="row" class="p-3 text-left">รวมทั้งโรงเรียน</th>
					<td class="p-3 text-right tabular-nums">{number.format(statistics.students.male)}</td>
					<td class="p-3 text-right tabular-nums">{number.format(statistics.students.female)}</td>
					<td class="p-3 text-right tabular-nums"
						>{number.format(statistics.students.otherOrUnspecified)}</td
					>
					<td class="p-3 text-right tabular-nums">{number.format(statistics.students.total)}</td>
				</tr></tfoot
			>
		</table>
	</div>
	<div class="mt-5 overflow-x-auto rounded-xl border bg-card" data-testid="school-grade-summary">
		<table class="w-full min-w-[520px] text-sm">
			<caption class="px-4 py-3 text-left font-medium">นักเรียนรายระดับชั้น</caption>
			<thead class="bg-muted/50 text-muted-foreground"
				><tr
					><th scope="col" class="p-3 text-left">ระดับชั้น</th><th
						scope="col"
						class="p-3 text-right">ห้อง</th
					><th scope="col" class="p-3 text-right">ชาย</th><th scope="col" class="p-3 text-right"
						>หญิง</th
					><th scope="col" class="p-3 text-right">อื่น ๆ / ไม่ระบุ</th><th
						scope="col"
						class="p-3 text-right">ทั้งหมด</th
					></tr
				></thead
			>
			<tbody class="divide-y divide-border">
				{#each statistics.grades as grade (`${grade.levelType}-${grade.year}`)}
					{@const key = `${grade.levelType}-${grade.year}`}
					<tr
						><th scope="row" class="p-3 text-left font-medium"
							><button
								class="flex items-center gap-2 rounded-md text-left hover:text-primary focus-visible:outline-2 focus-visible:outline-primary"
								aria-expanded={expandedGrades.includes(key)}
								onclick={() =>
									(expandedGrades = expandedGrades.includes(key)
										? expandedGrades.filter((item) => item !== key)
										: [...expandedGrades, key])}
								><ChevronDown
									class={`size-4 shrink-0 transition-transform ${expandedGrades.includes(key) ? 'rotate-180' : ''}`}
								/>{gradeName(grade.levelType, grade.year)}</button
							></th
						><td class="p-3 text-right">{number.format(grade.homerooms.length)}</td
						>{@render genderCells(grade.students)}</tr
					>
					{#if expandedGrades.includes(key)}<tr class="bg-muted/20"
							><td colspan="6" class="p-3"
								><table class="w-full text-left text-sm">
									<caption class="mb-2 text-left font-medium"
										>สถิติรายห้อง {gradeName(grade.levelType, grade.year)}</caption
									><thead
										><tr class="border-b"
											><th scope="col" class="p-3">ห้องเรียน</th><th
												scope="col"
												class="p-3 text-right">ชาย</th
											><th scope="col" class="p-3 text-right">หญิง</th><th
												scope="col"
												class="p-3 text-right">อื่น ๆ / ไม่ระบุ</th
											><th scope="col" class="p-3 text-right">ทั้งหมด</th></tr
										></thead
									><tbody class="divide-y divide-border">
										{#each grade.homerooms as room, index (index)}<tr
												><th scope="row" class="p-3 font-normal">{room.name}</th
												>{@render genderCells(room.students)}</tr
											>{/each}
										{#if grade.unassignedStudents.total}<tr
												><th scope="row" class="p-3 font-normal">ยังไม่ได้จัดห้อง</th
												>{@render genderCells(grade.unassignedStudents)}</tr
											>{/if}
									</tbody>
								</table></td
							></tr
						>{/if}
				{/each}
			</tbody><tfoot class="border-t bg-muted/50 font-semibold"
				><tr
					><th scope="row" class="p-3 text-left">รวมทั้งโรงเรียน</th><td class="p-3 text-right"
						>{number.format(statistics.totalHomerooms)}</td
					>{@render genderCells(statistics.students)}</tr
				></tfoot
			>
		</table>
	</div>
	{#if statistics.unassignedStudents.total > 0}<p class="mt-4 text-sm text-muted-foreground">
			ยอดรวมรวมนักเรียนที่ยังไม่ได้จัดห้อง {number.format(statistics.unassignedStudents.total)} คน
		</p>{/if}
{/if}

{#snippet genderCells(counts: PublicSchoolStatistics['students'])}
	<td class="p-3 text-right tabular-nums">{number.format(counts.male)}</td><td
		class="p-3 text-right tabular-nums">{number.format(counts.female)}</td
	><td class="p-3 text-right tabular-nums">{number.format(counts.otherOrUnspecified)}</td><td
		class="p-3 text-right font-semibold tabular-nums">{number.format(counts.total)}</td
	>
{/snippet}
