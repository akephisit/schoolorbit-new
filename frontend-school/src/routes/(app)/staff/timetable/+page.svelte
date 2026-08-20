<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		type TimetableEntry,
		type TimetablePeriodSummary,
		getMyTimetable
	} from '$lib/api/timetable';
	import {
		getAcademicStructure,
		getSchoolDays,
		type AcademicYear,
		type Semester
	} from '$lib/api/academic';
	import { authStore } from '$lib/stores/auth';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import { generateTimetablePDF } from '$lib/utils/pdf';
	import {
		buildStaffOwnTimetablePdfDownload,
		canDownloadStaffOwnTimetablePdf,
		runStaffOwnTimetablePdfDownload,
		staffOwnTimetableSelectionKey
	} from '$lib/utils/staff-own-timetable-pdf';
	import { Download, Loader2, School, MapPin } from 'lucide-svelte';

	let loading = $state(true);
	let isExportingPdf = $state(false);
	let loadedSelectionKey = $state('');
	let entries = $state<TimetableEntry[]>([]);
	let periods = $state<TimetablePeriodSummary[]>([]);
	let years = $state<AcademicYear[]>([]);
	let semesters = $state<Semester[]>([]);
	let selectedYearId = $state('');
	let selectedSemesterId = $state('');
	let schoolDays = $state<{ value: string; label: string; shortLabel: string }[]>([]);
	let timetableLoadRequest = 0;

	const userId = $derived($authStore.user?.id ?? '');
	const userName = $derived(
		$authStore.user
			? `${$authStore.user.firstName ?? ''} ${$authStore.user.lastName ?? ''}`.trim()
			: ''
	);

	const semestersOfYear = $derived(semesters.filter((s) => s.academic_year_id === selectedYearId));
	const selectedSemester = $derived(
		semesters.find((semester) => semester.id === selectedSemesterId)
	);
	const canDownloadPdf = $derived(
		canDownloadStaffOwnTimetablePdf({
			loading,
			isExporting: isExportingPdf,
			selectedYearId,
			selectedSemesterId,
			selectedSemesterYearId: selectedSemester?.academic_year_id,
			loadedSelectionKey,
			entryCount: entries.length,
			periodCount: periods.length
		})
	);

	// คอลัมน์วัน 80px + คาบละ 110px → mobile บีบไม่ได้ ต้องเลื่อน
	const tableMinWidth = $derived(80 + periods.length * 110);

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
		const requestId = ++timetableLoadRequest;
		const semester = semesters.find((item) => item.id === selectedSemesterId);
		const selectionKey = staffOwnTimetableSelectionKey(selectedYearId, selectedSemesterId);

		if (!selectionKey || !userId || semester?.academic_year_id !== selectedYearId) {
			loadedSelectionKey = '';
			periods = [];
			entries = [];
			if (userId && selectedYearId && selectedSemesterId) loading = false;
			return;
		}

		try {
			loading = true;
			loadedSelectionKey = '';
			periods = [];
			entries = [];
			const year = years.find((item) => item.id === selectedYearId);
			if (year) schoolDays = getSchoolDays(year.school_days);

			const entriesRes = await getMyTimetable({
				academic_semester_id: selectedSemesterId,
				include_team_ghosts: true
			});
			if (requestId !== timetableLoadRequest) return;

			entries = entriesRes.data;
			periods = entriesRes.periods;
			loadedSelectionKey = selectionKey;
		} catch (e: unknown) {
			if (requestId !== timetableLoadRequest) return;
			loadedSelectionKey = '';
			periods = [];
			entries = [];
			console.error(e);
			toast.error('โหลดตารางสอนไม่สำเร็จ');
		} finally {
			if (requestId === timetableLoadRequest) loading = false;
		}
	}

	function getEntry(day: string, periodId: string): TimetableEntry | undefined {
		return entries.find((e) => e.day_of_week === day && e.period_id === periodId);
	}

	async function handleDownloadPDF() {
		if (!canDownloadPdf) return;

		const selectedYear = years.find((year) => year.id === selectedYearId);
		const download = buildStaffOwnTimetablePdfDownload({
			teacherName: userName,
			semesterName: selectedSemester?.name,
			semesterTerm: selectedSemester?.term,
			academicYearName: selectedYear?.name,
			entries,
			dayValues: schoolDays.map((day) => day.value),
			periods
		});

		await runStaffOwnTimetablePdfDownload(download, {
			generatePdf: generateTimetablePDF,
			setExporting: (value) => (isExportingPdf = value),
			onSuccess: () => toast.success('ดาวน์โหลดตารางสอนแล้ว'),
			onError: (error) => {
				console.error('Failed to download timetable PDF', error);
				toast.error('ดาวน์โหลดตารางสอนไม่สำเร็จ');
			}
		});
	}

	$effect(() => {
		// Auto-reload เมื่อเปลี่ยน semester หรือ user ready
		if (selectedSemesterId && selectedYearId && userId) {
			loadPeriodsAndEntries();
		}
	});

	onMount(loadStructure);
</script>

<PageShell title="ตารางสอน" description={userName ? `ครู${userName}` : 'ตารางสอนของฉัน'}>
	{#snippet actions()}
		<Button variant="outline" onclick={handleDownloadPDF} disabled={!canDownloadPdf}>
			{#if isExportingPdf}
				<Loader2 class="mr-2 h-4 w-4 animate-spin" />
			{:else}
				<Download class="mr-2 h-4 w-4" />
			{/if}
			ดาวน์โหลด PDF
		</Button>
	{/snippet}

	<!-- Year + Semester selector -->
	<div class="flex flex-wrap gap-3 rounded-xl border bg-card p-3 sm:p-4">
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
		<PageSkeleton variant="table" rows={6} columns={Math.max(periods.length + 1, 4)} />
	{:else if periods.length === 0}
		<PageState
			title="ยังไม่มีคาบเรียนที่เปิดใช้งานในปีการศึกษานี้"
			description="เมื่อโรงเรียนตั้งค่าคาบเรียนแล้ว ตารางสอนจะแสดงครบทุกคาบในหน้านี้"
		/>
	{:else}
		<!-- Timetable Grid (วัน=แถว, คาบ=คอลัมน์) -->
		<div class="overflow-x-auto">
			<table class="w-full table-fixed border-collapse" style="min-width: {tableMinWidth}px">
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
										{@const isCourse = entry.entry_type === 'COURSE'}
										<div
											class="flex h-full w-full flex-col gap-0.5 rounded border p-2 text-xs {getEntryColor(
												entry.entry_type
											)} {isCourse ? 'text-left' : 'items-center justify-center text-center'}"
										>
											<div
												class="w-full font-semibold {isCourse
													? 'truncate'
													: 'line-clamp-3 leading-tight whitespace-pre-line'}"
											>
												{entry.subject_code || entry.title || entry.subject_name_th || ''}
											</div>
											{#if isCourse && entry.subject_name_th}
												<div class="w-full truncate text-[10px] opacity-80">
													{entry.subject_name_th}
												</div>
											{/if}
											{#if isCourse && entry.classroom_name}
												<div
													class="mt-auto flex w-full items-center gap-1 truncate text-[10px] opacity-70"
												>
													<School class="h-2.5 w-2.5 shrink-0" />
													{entry.classroom_name}
												</div>
											{/if}
											{#if isCourse && entry.room_code}
												<div class="flex w-full items-center gap-1 text-[10px] opacity-60">
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
</PageShell>
