<script lang="ts">
	import type { PublicSchoolStatistics } from '#lib/api/school.js';
	import { summarizeEducationLevels } from '#lib/school-public/statistics.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { SvelteSet } from 'svelte/reactivity';
	import { GraduationCap, BookOpen, UsersRound, DoorOpen, ChevronRight } from '@lucide/svelte';
	let { statistics }: { statistics: PublicSchoolStatistics } = $props();
	const id = $props.id();
	const expandedGrades = new SvelteSet<string>();
	function toggleGrade(key: string) {
		if (expandedGrades.has(key)) expandedGrades.delete(key);
		else expandedGrades.add(key);
	}
	const number = new Intl.NumberFormat('th-TH');
	const showOther = $derived(statistics.students.otherOrUnspecified > 0);
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
			{statistics.academicYear?.name ?? 'ยังไม่มีปีการศึกษาที่เปิดใช้งาน'} · กดระดับชั้นเพื่อดูรายห้อง
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
	<div class="mt-5 overflow-x-auto rounded-xl border bg-card" data-testid="school-student-summary">
		<table class="w-full min-w-96 text-sm">
			<caption class="px-4 py-3 text-left font-medium">สรุปจำนวนนักเรียนรายชั้นและห้อง</caption>
			<thead class="bg-primary text-primary-foreground">
				<tr
					><th scope="col" class="p-3 text-left">ชั้น / ห้อง</th><th
						scope="col"
						class="p-3 text-right">ชาย</th
					><th scope="col" class="p-3 text-right">หญิง</th>{#if showOther}<th
							scope="col"
							class="p-3 text-right">อื่น ๆ / ไม่ระบุ</th
						>{/if}<th scope="col" class="p-3 text-right">รวม</th></tr
				>
			</thead>
			{#each educationLevels as level (level.id)}
				{#each level.grades as grade (`${grade.levelType}-${grade.year}`)}
					{@const key = `${grade.levelType}-${grade.year}`}
					<tbody>
						<tr class="border-t bg-primary/15 font-semibold">
							<th scope="row" class="p-1 text-left">
								{#if grade.homerooms.length || grade.unassignedStudents.total}
									<Button
										variant="ghost"
										class="h-auto min-h-10 justify-start gap-2 px-2 py-2 font-semibold"
										aria-expanded={expandedGrades.has(key)}
										aria-controls={`${id}-${key}`}
										onclick={() => toggleGrade(key)}
									>
										<ChevronRight
											class={[
												'size-4 shrink-0 transition-transform motion-reduce:transition-none',
												expandedGrades.has(key) && 'rotate-90'
											]}
										/>
										{gradeName(grade.levelType, grade.year)}
									</Button>
								{:else}<span class="block p-2">{gradeName(grade.levelType, grade.year)}</span>{/if}
							</th>
							{@render genderCells(grade.students)}
						</tr>
					</tbody>
					<tbody
						id={`${id}-${key}`}
						hidden={!expandedGrades.has(key)}
						class="divide-y divide-border"
					>
						{#each [...grade.homerooms].sort( (a, b) => a.name.localeCompare( b.name, 'th', { numeric: true } ) ) as room, index (index)}
							<tr class="bg-primary/5"
								><th scope="row" class="px-4 py-2 text-left font-normal">{room.name}</th
								>{@render genderCells(room.students)}</tr
							>
						{/each}
						{#if grade.unassignedStudents.total}
							<tr class="bg-primary/5"
								><th scope="row" class="px-4 py-2 text-left font-normal"
									>{gradeName(grade.levelType, grade.year)} · ยังไม่ได้จัดห้อง</th
								>{@render genderCells(grade.unassignedStudents)}</tr
							>
						{/if}
					</tbody>
				{/each}
				<tbody
					><tr class="border-t bg-primary/25 font-semibold"
						><th scope="row" class="p-3 text-left">รวม{level.label}</th>{@render genderCells(
							level.students
						)}</tr
					>
				</tbody>
			{/each}
			<tfoot class="border-t bg-primary text-primary-foreground font-semibold"
				><tr
					><th scope="row" class="p-3 text-left">รวมทั้งโรงเรียน</th>{@render genderCells(
						statistics.students
					)}</tr
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
	>{#if showOther}<td class="p-3 text-right tabular-nums"
			>{number.format(counts.otherOrUnspecified)}</td
		>{/if}<td class="p-3 text-right font-semibold tabular-nums">{number.format(counts.total)}</td>
{/snippet}
