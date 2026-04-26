<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		type TimetableEntry,
		type AcademicPeriod,
		listTimetableEntries,
		listPeriods
	} from '$lib/api/timetable';
	import {
		getAcademicStructure,
		getSchoolDays,
		type AcademicYear,
		type Semester
	} from '$lib/api/academic';
	import { authStore } from '$lib/stores/auth';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import { CalendarDays, LoaderCircle, School, MapPin } from 'lucide-svelte';

	let { data } = $props();

	let loading = $state(true);
	let entries = $state<TimetableEntry[]>([]);
	let periods = $state<AcademicPeriod[]>([]);
	let years = $state<AcademicYear[]>([]);
	let semesters = $state<Semester[]>([]);
	let selectedYearId = $state('');
	let selectedSemesterId = $state('');
	let schoolDays = $state<{ value: string; label: string; shortLabel: string }[]>([]);

	const userId = $derived($authStore.user?.id ?? '');
	const userName = $derived(
		$authStore.user
			? `${$authStore.user.firstName ?? ''} ${$authStore.user.lastName ?? ''}`.trim()
			: ''
	);

	const semestersOfYear = $derived(semesters.filter((s) => s.academic_year_id === selectedYearId));

	function formatTime(t?: string): string {
		if (!t) return '';
		return t.substring(0, 5);
	}

	function getEntryColor(type: string): string {
		if (type === 'COURSE') return 'bg-blue-50 border-blue-200 text-blue-900';
		if (type === 'ACTIVITY') return 'bg-emerald-50 border-emerald-200 text-emerald-900';
		if (type === 'BREAK') return 'bg-amber-50 border-amber-200 text-amber-800';
		if (type === 'HOMEROOM') return 'bg-purple-50 border-purple-200 text-purple-900';
		if (type === 'ACADEMIC') return 'bg-blue-50 border-blue-200 text-blue-900';
		return 'bg-gray-50 border-gray-200 text-gray-900';
	}

	async function loadStructure() {
		try {
			const structRes = await getAcademicStructure();
			years = structRes.data.years;
			semesters = structRes.data.semesters;

			const activeYear = years.find((y) => y.is_active) ?? years[0];
			if (activeYear) {
				selectedYearId = activeYear.id;
				schoolDays = getSchoolDays(activeYear.school_days);
				const activeSem =
					semesters.find((s) => s.academic_year_id === activeYear.id && s.is_active) ??
					semesters.find((s) => s.academic_year_id === activeYear.id);
				if (activeSem) {
					selectedSemesterId = activeSem.id;
				}
			}
		} catch (e: unknown) {
			console.error(e);
			toast.error((e instanceof Error ? e.message : String(e)) || 'โหลดข้อมูลไม่สำเร็จ');
		}
	}

	async function loadPeriodsAndEntries() {
		if (!selectedYearId || !selectedSemesterId || !userId) {
			periods = [];
			entries = [];
			return;
		}
		try {
			loading = true;
			const [periodsRes, entriesRes] = await Promise.all([
				listPeriods({ academic_year_id: selectedYearId, active_only: true }),
				listTimetableEntries({
					instructor_id: userId,
					academic_semester_id: selectedSemesterId
				})
			]);
			periods = periodsRes.data.sort((a, b) => a.order_index - b.order_index);
			entries = entriesRes.data;
			// อัปเดต schoolDays ตาม year ที่เลือก
			const year = years.find((y) => y.id === selectedYearId);
			if (year) schoolDays = getSchoolDays(year.school_days);
		} catch (e: unknown) {
			console.error(e);
			toast.error('โหลดตารางสอนไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	function getEntry(day: string, periodId: string): TimetableEntry | undefined {
		return entries.find((e) => e.day_of_week === day && e.period_id === periodId);
	}

	$effect(() => {
		// Auto-reload เมื่อเปลี่ยน semester หรือ user ready
		if (selectedSemesterId && selectedYearId && userId) {
			loadPeriodsAndEntries();
		}
	});

	onMount(loadStructure);
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<div class="container mx-auto space-y-4 p-4 md:p-6">
	<div class="flex items-center gap-3">
		<CalendarDays class="text-primary h-7 w-7" />
		<div>
			<h1 class="text-2xl font-bold">ตารางสอน</h1>
			{#if userName}
				<p class="text-muted-foreground text-sm">ครู{userName}</p>
			{/if}
		</div>
	</div>

	<!-- Year + Semester selector -->
	<div class="flex flex-wrap gap-3">
		<div class="w-[220px]">
			<Select.Root type="single" bind:value={selectedYearId}>
				<Select.Trigger class="w-full">
					{years.find((y) => y.id === selectedYearId)?.name || 'เลือกปีการศึกษา'}
				</Select.Trigger>
				<Select.Content>
					{#each years as year (year.id)}
						<Select.Item value={year.id}>{year.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
		<div class="w-[200px]">
			<Select.Root type="single" bind:value={selectedSemesterId}>
				<Select.Trigger class="w-full">
					{semestersOfYear.find((s) => s.id === selectedSemesterId)?.name || 'เลือกภาคเรียน'}
				</Select.Trigger>
				<Select.Content>
					{#each semestersOfYear as sem (sem.id)}
						<Select.Item value={sem.id}>{sem.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<LoaderCircle class="text-muted-foreground h-8 w-8 animate-spin" />
		</div>
	{:else if periods.length === 0}
		<Card class="text-muted-foreground p-8 text-center">
			<CalendarDays class="mx-auto mb-3 h-12 w-12 opacity-30" />
			<p>ยังไม่มีคาบเรียนที่ตั้งค่าในปีการศึกษานี้</p>
			<Button variant="outline" size="sm" class="mt-3" href="/staff/academic/periods">
				ไปตั้งค่าคาบเรียน
			</Button>
		</Card>
	{:else if entries.length === 0}
		<Card class="text-muted-foreground p-8 text-center">
			<CalendarDays class="mx-auto mb-3 h-12 w-12 opacity-30" />
			<p>ยังไม่มีตารางสอนของคุณในภาคเรียนนี้</p>
		</Card>
	{:else}
		<!-- Timetable Grid (วัน=แถว, คาบ=คอลัมน์) -->
		<div class="overflow-x-auto">
			<table class="w-full min-w-[640px] table-fixed border-collapse">
				<thead>
					<tr>
						<th class="bg-muted/50 text-muted-foreground w-20 border p-2 text-xs font-medium">
							วัน / คาบ
						</th>
						{#each periods as period (period.id)}
							<th class="bg-muted/50 border p-2 text-center text-xs font-medium">
								<div class="font-semibold">{period.name || ' '}</div>
								<div class="text-muted-foreground text-[10px] font-normal">
									{formatTime(period.start_time)}-{formatTime(period.end_time)}
								</div>
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each schoolDays as day (day.value)}
						<tr>
							<td class="bg-muted/30 border p-2 text-center text-xs font-medium">
								{day.label}
							</td>
							{#each periods as period (period.id)}
								{@const entry = getEntry(day.value, period.id)}
								<td class="relative h-20 border p-1">
									{#if entry}
										<div
											class="flex h-full w-full flex-col gap-0.5 rounded border p-2 text-left text-xs {getEntryColor(
												entry.entry_type
											)}"
										>
											<div class="truncate font-semibold">
												{entry.subject_code || entry.title || entry.subject_name_th || ''}
											</div>
											{#if entry.entry_type === 'COURSE' && entry.subject_name_th}
												<div class="truncate text-[10px] opacity-80">
													{entry.subject_name_th}
												</div>
											{/if}
											{#if entry.classroom_name}
												<div
													class="mt-auto flex items-center gap-1 truncate text-[10px] opacity-70"
												>
													<School class="h-2.5 w-2.5 shrink-0" />
													{entry.classroom_name}
												</div>
											{/if}
											{#if entry.room_code}
												<div class="flex items-center gap-1 text-[10px] opacity-60">
													<MapPin class="h-2.5 w-2.5 shrink-0" />
													{entry.room_code}
												</div>
											{/if}
										</div>
									{/if}
								</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>

		<!-- Legend -->
		<div class="text-muted-foreground flex flex-wrap gap-3 text-xs">
			<div class="flex items-center gap-1.5">
				<div class="h-3 w-3 rounded border border-blue-200 bg-blue-100"></div>
				วิชาเรียน
			</div>
			<div class="flex items-center gap-1.5">
				<div class="h-3 w-3 rounded border border-emerald-200 bg-emerald-100"></div>
				กิจกรรม
			</div>
			<div class="flex items-center gap-1.5">
				<div class="h-3 w-3 rounded border border-amber-200 bg-amber-100"></div>
				พัก
			</div>
			<div class="flex items-center gap-1.5">
				<div class="h-3 w-3 rounded border border-purple-200 bg-purple-100"></div>
				โฮมรูม
			</div>
		</div>
	{/if}
</div>
