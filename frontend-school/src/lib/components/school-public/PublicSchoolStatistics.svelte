<script lang="ts">
	import type { PublicSchoolStatistics } from '$lib/api/school';
	import { PageState } from '$lib/components/app-state';
	import { GraduationCap, BookOpen, UsersRound, DoorOpen, ChevronDown } from '@lucide/svelte';
	let { statistics }: { statistics: PublicSchoolStatistics } = $props();
	const number = new Intl.NumberFormat('th-TH');
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
			{statistics.academicYear?.name ?? 'ยังไม่มีปีการศึกษาที่เปิดใช้งาน'} · กางแต่ละชั้นเพื่อดูรายห้อง
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
	<div class="mt-5 flex flex-wrap gap-3 text-sm">
		<span class="rounded-full bg-primary/10 px-4 py-2 text-primary"
			>ชาย {number.format(statistics.students.male)} คน</span
		>
		<span class="rounded-full bg-secondary px-4 py-2"
			>หญิง {number.format(statistics.students.female)} คน</span
		>
		<span class="rounded-full bg-muted px-4 py-2 text-muted-foreground"
			>อื่น ๆ / ไม่ระบุ {number.format(statistics.students.otherOrUnspecified)} คน</span
		>
	</div>
	<div class="mt-5 space-y-3">
		{#each statistics.grades as grade (`${grade.levelType}-${grade.year}`)}
			<details class="public-surface group overflow-hidden rounded-xl border border-border bg-card">
				<summary
					class="flex cursor-pointer list-none flex-wrap items-center justify-between gap-3 p-4 sm:p-5"
				>
					<div>
						<h4 class="inline text-base font-medium">{gradeName(grade.levelType, grade.year)}</h4>
						<p class="mt-1 text-xs text-muted-foreground">
							{grade.homerooms.length} ห้อง · ชาย {number.format(grade.students.male)} · หญิง {number.format(
								grade.students.female
							)} · อื่น ๆ / ไม่ระบุ {number.format(grade.students.otherOrUnspecified)}
						</p>
					</div>
					<div class="flex items-center gap-4">
						<span class="font-semibold"
							>{number.format(grade.students.total)}
							<span class="text-xs font-normal text-muted-foreground">คน</span></span
						><ChevronDown
							class="size-4 text-muted-foreground transition-transform group-open:rotate-180"
						/>
					</div>
				</summary>
				<div class="overflow-x-auto border-t border-border">
					<table class="w-full min-w-[480px] text-left text-sm">
						<caption class="sr-only">สถิติรายห้อง {gradeName(grade.levelType, grade.year)}</caption>
						<thead class="bg-muted/50 text-muted-foreground"
							><tr
								><th scope="col" class="px-5 py-3 font-medium">ห้องเรียน</th><th
									scope="col"
									class="px-3 py-3 text-right font-medium">ชาย</th
								><th scope="col" class="px-3 py-3 text-right font-medium">หญิง</th><th
									scope="col"
									class="px-3 py-3 text-right font-medium">อื่น ๆ / ไม่ระบุ</th
								><th scope="col" class="px-5 py-3 text-right font-medium">รวม</th></tr
							></thead
						>
						<tbody class="divide-y divide-border">
							{#each grade.homerooms as room, index (index)}
								<tr
									><th scope="row" class="px-5 py-3 font-normal">{room.name}</th><td
										class="px-3 py-3 text-right">{number.format(room.students.male)}</td
									><td class="px-3 py-3 text-right">{number.format(room.students.female)}</td><td
										class="px-3 py-3 text-right"
										>{number.format(room.students.otherOrUnspecified)}</td
									><td class="px-5 py-3 text-right font-medium"
										>{number.format(room.students.total)}</td
									></tr
								>
							{/each}
							{#if grade.unassignedStudents.total > 0}
								<tr class="bg-muted/30"
									><th scope="row" class="px-5 py-3 font-normal">ยังไม่ได้จัดห้อง</th><td
										class="px-3 py-3 text-right">{number.format(grade.unassignedStudents.male)}</td
									><td class="px-3 py-3 text-right"
										>{number.format(grade.unassignedStudents.female)}</td
									><td class="px-3 py-3 text-right"
										>{number.format(grade.unassignedStudents.otherOrUnspecified)}</td
									><td class="px-5 py-3 text-right font-medium"
										>{number.format(grade.unassignedStudents.total)}</td
									></tr
								>
							{/if}
						</tbody>
					</table>
				</div>
			</details>
		{/each}
	</div>
	{#if statistics.unassignedStudents.total > 0}<p class="mt-4 text-sm text-muted-foreground">
			ยอดรวมรวมนักเรียนที่ยังไม่ได้จัดห้อง {number.format(statistics.unassignedStudents.total)} คน
		</p>{/if}
{/if}
