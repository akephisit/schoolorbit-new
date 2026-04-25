<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		type TimetableEntry,
		type AcademicPeriod,
		type MyActivityForEntry,
		listTimetableEntries,
		listPeriods,
		getMyActivityForEntry
	} from '$lib/api/timetable';
	import {
		getAcademicStructure,
		getSchoolDays,
		type AcademicYear,
		type Semester
	} from '$lib/api/academic';
	import { getOwnProfile, type Student } from '$lib/api/students';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Card } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { CalendarDays, Loader2, Users, BookOpen } from 'lucide-svelte';

	let loading = $state(true);
	let student = $state<Student | null>(null);
	let entries = $state<TimetableEntry[]>([]);
	let periods = $state<AcademicPeriod[]>([]);
	let years = $state<AcademicYear[]>([]);
	let semesters = $state<Semester[]>([]);
	let selectedSemesterId = $state('');
	let schoolDays = $state<{ value: string; label: string; shortLabel: string }[]>([]);

	// Activity detail dialog
	let showActivityDetail = $state(false);
	let activityLoading = $state(false);
	let activityData = $state<MyActivityForEntry | null>(null);
	let activityEntryTitle = $state('');

	function formatTime(t?: string): string {
		if (!t) return '';
		return t.substring(0, 5);
	}

	function getEntryColor(type: string): string {
		if (type === 'COURSE') return 'bg-blue-50 border-blue-200 text-blue-900';
		if (type === 'ACTIVITY') return 'bg-emerald-50 border-emerald-200 text-emerald-900';
		if (type === 'BREAK') return 'bg-amber-50 border-amber-200 text-amber-800';
		if (type === 'HOMEROOM') return 'bg-purple-50 border-purple-200 text-purple-900';
		return 'bg-gray-50 border-gray-200 text-gray-900';
	}

	async function loadData() {
		try {
			const [profileRes, structRes] = await Promise.all([getOwnProfile(), getAcademicStructure()]);

			student = profileRes.data;
			years = structRes.data.years;
			semesters = structRes.data.semesters;

			// Find active year + semester
			const activeYear = years.find((y) => y.is_active);
			if (activeYear) {
				schoolDays = getSchoolDays(activeYear.school_days);
				const activeSem = semesters.find(
					(s) => s.academic_year_id === activeYear.id && s.is_active
				);
				if (activeSem) {
					selectedSemesterId = activeSem.id;
					// Load periods
					const periodsRes = await listPeriods({
						academic_year_id: activeYear.id,
						active_only: true
					});
					periods = periodsRes.data;
					await loadTimetable();
				}
			}
		} catch (e: unknown) {
			console.error(e);
			toast.error((e instanceof Error ? e.message : String(e)) || 'โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function loadTimetable() {
		if (!student || !selectedSemesterId) return;
		try {
			const res = await listTimetableEntries({
				student_id: student.id,
				academic_semester_id: selectedSemesterId
			});
			entries = res.data;
		} catch (e: unknown) {
			console.error(e);
			toast.error('โหลดตารางเรียนไม่สำเร็จ');
		}
	}

	function getEntry(day: string, periodId: string): TimetableEntry | undefined {
		return entries.find((e) => e.day_of_week === day && e.period_id === periodId);
	}

	async function handleActivityClick(entry: TimetableEntry) {
		if (!entry.activity_slot_id) return;
		activityEntryTitle = entry.title || entry.activity_slot_name || 'กิจกรรม';
		showActivityDetail = true;
		activityLoading = true;
		activityData = null;
		try {
			const res = await getMyActivityForEntry(entry.id);
			activityData = res.data;
		} catch (e: unknown) {
			console.error(e);
			toast.error('โหลดข้อมูลกิจกรรมไม่สำเร็จ');
		} finally {
			activityLoading = false;
		}
	}

	onMount(loadData);
</script>

<svelte:head>
	<title>ตารางเรียน</title>
</svelte:head>

<div class="container mx-auto p-4 md:p-6 space-y-4">
	<div class="flex items-center gap-3">
		<CalendarDays class="w-7 h-7 text-primary" />
		<div>
			<h1 class="text-2xl font-bold">ตารางเรียน</h1>
			{#if student}
				<p class="text-sm text-muted-foreground">
					{student.first_name}
					{student.last_name}
					{#if student.grade_level && student.class_room}
						— {student.grade_level}/{student.class_room}
					{/if}
				</p>
			{/if}
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<Loader2 class="w-8 h-8 animate-spin text-muted-foreground" />
		</div>
	{:else if periods.length === 0}
		<Card class="p-8 text-center text-muted-foreground">
			<CalendarDays class="w-12 h-12 mx-auto mb-3 opacity-30" />
			<p>ยังไม่มีตารางเรียนในภาคเรียนนี้</p>
		</Card>
	{:else}
		<!-- Timetable Grid -->
		<div class="overflow-x-auto">
			<table class="w-full border-collapse min-w-[640px]">
				<thead>
					<tr>
						<th class="p-2 border bg-muted/50 text-xs font-medium text-muted-foreground w-20">
							คาบ
						</th>
						{#each schoolDays as day (day.value)}
							<th class="p-2 border bg-muted/50 text-xs font-medium text-center">
								{day.label}
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each periods as period (period.id)}
						<tr>
							<td class="p-2 border bg-muted/30 text-center">
								<div class="text-xs font-medium">{period.name || ' '}</div>
								<div class="text-[10px] text-muted-foreground">
									{formatTime(period.start_time)}-{formatTime(period.end_time)}
								</div>
							</td>
							{#each schoolDays as day (day.value)}
								{@const entry = getEntry(day.value, period.id)}
								<td class="p-1 border relative h-20">
									{#if entry}
										{@const isClickable =
											entry.entry_type === 'ACTIVITY' && !!entry.activity_slot_id}
										<button
											class="w-full h-full rounded border p-2 text-left text-xs flex flex-col gap-0.5 transition-all {getEntryColor(
												entry.entry_type
											)} {isClickable
												? 'cursor-pointer hover:shadow-md hover:brightness-95'
												: 'cursor-default'}"
											onclick={() => isClickable && handleActivityClick(entry)}
											disabled={!isClickable}
										>
											<div class="font-semibold truncate">
												{entry.subject_code || entry.title || entry.subject_name_th || ''}
											</div>
											{#if entry.entry_type === 'COURSE' && entry.subject_name_th}
												<div class="truncate text-[10px] opacity-80">
													{entry.subject_name_th}
												</div>
											{/if}
											{#if entry.instructor_name}
												<div class="truncate text-[10px] opacity-70 mt-auto">
													{entry.instructor_name}
												</div>
											{/if}
											{#if entry.room_code}
												<div class="text-[10px] opacity-60">{entry.room_code}</div>
											{/if}
											{#if isClickable}
												<Badge
													variant="outline"
													class="text-[9px] px-1 py-0 mt-0.5 w-fit border-emerald-300 text-emerald-700"
												>
													กดดูกิจกรรม
												</Badge>
											{/if}
										</button>
									{/if}
								</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>

		<!-- Legend -->
		<div class="flex flex-wrap gap-3 text-xs text-muted-foreground">
			<div class="flex items-center gap-1.5">
				<div class="w-3 h-3 rounded bg-blue-100 border border-blue-200"></div>
				วิชาเรียน
			</div>
			<div class="flex items-center gap-1.5">
				<div class="w-3 h-3 rounded bg-emerald-100 border border-emerald-200"></div>
				กิจกรรม
			</div>
			<div class="flex items-center gap-1.5">
				<div class="w-3 h-3 rounded bg-amber-100 border border-amber-200"></div>
				พัก
			</div>
			<div class="flex items-center gap-1.5">
				<div class="w-3 h-3 rounded bg-purple-100 border border-purple-200"></div>
				โฮมรูม
			</div>
		</div>
	{/if}
</div>

<!-- Activity Detail Dialog -->
<Dialog.Root bind:open={showActivityDetail}>
	<Dialog.Content class="sm:max-w-[420px]">
		<Dialog.Header>
			<Dialog.Title>{activityEntryTitle}</Dialog.Title>
			<Dialog.Description>รายละเอียดกิจกรรมที่ลงทะเบียน</Dialog.Description>
		</Dialog.Header>

		{#if activityLoading}
			<div class="flex items-center justify-center py-8">
				<Loader2 class="w-6 h-6 animate-spin text-muted-foreground" />
			</div>
		{:else if activityData}
			{#if activityData.enrolled}
				<div class="space-y-4 py-2">
					<div class="flex items-start gap-3">
						<BookOpen class="w-5 h-5 text-emerald-600 mt-0.5 shrink-0" />
						<div>
							<p class="font-semibold text-sm">{activityData.group_name}</p>
							<Badge variant="default" class="mt-1 text-xs bg-emerald-600">ลงทะเบียนแล้ว</Badge>
						</div>
					</div>

					{#if activityData.instructors && activityData.instructors.length > 0}
						<div class="flex items-start gap-3">
							<Users class="w-5 h-5 text-blue-600 mt-0.5 shrink-0" />
							<div>
								<p class="text-xs text-muted-foreground mb-1">ครูผู้สอน</p>
								{#each activityData.instructors as instr (instr.id)}
									<p class="text-sm">{instr.name}</p>
								{/each}
							</div>
						</div>
					{/if}

					<div class="flex items-start gap-3">
						<Users class="w-5 h-5 text-orange-500 mt-0.5 shrink-0" />
						<div>
							<p class="text-xs text-muted-foreground mb-1">สมาชิก</p>
							<p class="text-sm">
								{activityData.member_count}{#if activityData.max_capacity}/{activityData.max_capacity}{/if}
								คน
							</p>
						</div>
					</div>
				</div>
			{:else}
				<div class="py-6 text-center text-muted-foreground">
					<BookOpen class="w-10 h-10 mx-auto mb-2 opacity-30" />
					<p class="text-sm">ยังไม่ได้ลงทะเบียนกิจกรรมในช่วงเวลานี้</p>
					<Button variant="outline" size="sm" class="mt-3" href="/student/activities">
						ไปลงทะเบียนกิจกรรม
					</Button>
				</div>
			{/if}
		{:else}
			<div class="py-6 text-center text-muted-foreground">
				<p class="text-sm">ไม่พบข้อมูลกิจกรรม</p>
			</div>
		{/if}

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showActivityDetail = false)}>ปิด</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
