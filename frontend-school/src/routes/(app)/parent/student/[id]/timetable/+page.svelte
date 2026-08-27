<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import {
		listChildAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import {
		resolveScopedAcademicYearUrl,
		urlWithAcademicYear
	} from '$lib/academic-context/scoped-year';
	import { getChildProfile, getChildTimetable } from '$lib/api/parents';
	import type { Student } from '$lib/api/students';
	import {
		periodsFromTimetableEntries,
		type TimetableEntry,
		type TimetablePeriodSummary
	} from '$lib/api/timetable';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import ScopedAcademicYearSelect from '$lib/components/academic-context/ScopedAcademicYearSelect.svelte';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { MapPin, School } from 'lucide-svelte';

	const dayOptions = [
		{ value: 'MON', label: 'จันทร์' },
		{ value: 'TUE', label: 'อังคาร' },
		{ value: 'WED', label: 'พุธ' },
		{ value: 'THU', label: 'พฤหัสบดี' },
		{ value: 'FRI', label: 'ศุกร์' },
		{ value: 'SAT', label: 'เสาร์' },
		{ value: 'SUN', label: 'อาทิตย์' }
	];

	let { params }: PageProps = $props();
	const studentId = $derived(params.id);
	let child = $state<Student | null>(null);
	let contextOptions = $state<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let selectedTermId = $state('');
	let periods = $state<TimetablePeriodSummary[]>([]);
	let entries = $state<TimetableEntry[]>([]);
	let loading = $state(true);
	let errorMessage = $state('');
	let revision = 0;

	const termOptions = $derived(
		contextOptions?.terms.filter((term) => term.academicYearId === selectedYearId) ?? []
	);
	const schoolDays = $derived.by(() => {
		const configured = new Set(entries.map((entry) => entry.dayOfWeek));
		return configured.size > 0
			? dayOptions.filter((day) => configured.has(day.value))
			: dayOptions.slice(0, 5);
	});
	const tableMinWidth = $derived(96 + periods.length * 132);
	const childName = $derived(
		child ? `${child.title ?? ''}${child.first_name} ${child.last_name}`.trim() : ''
	);
	const childDetailHref = $derived(
		`${resolve(`/parent/student/${studentId}`)}${selectedYearId ? `?academicYearId=${encodeURIComponent(selectedYearId)}` : ''}`
	);

	function authorizedSelection(options: AcademicContextOptionsResponse): {
		yearId: string;
		termId: string;
		replaceUrl: URL | null;
	} {
		const yearResolution = resolveScopedAcademicYearUrl(options, page.url);
		const yearId = yearResolution.academicYearId ?? '';
		if (!yearId) return { yearId: '', termId: '', replaceUrl: null };

		const terms = options.terms.filter((term) => term.academicYearId === yearId);
		const queryTermId = page.url.searchParams.get('academicTermId');
		const termId =
			terms.find((term) => term.id === queryTermId)?.id ??
			terms.find((term) => term.id === options.activeAcademicTermId)?.id ??
			terms[0]?.id ??
			'';

		const nextUrl = yearResolution.replaceUrl ?? new URL(page.url);
		if (termId) nextUrl.searchParams.set('academicTermId', termId);
		else nextUrl.searchParams.delete('academicTermId');
		return {
			yearId,
			termId,
			replaceUrl: nextUrl.href === page.url.href ? null : nextUrl
		};
	}

	async function loadHistory(): Promise<void> {
		const current = ++revision;
		loading = true;
		errorMessage = '';
		try {
			const options = await listChildAcademicContextOptions(studentId);
			if (current !== revision) return;
			contextOptions = options;
			const selection = authorizedSelection(options);
			selectedYearId = selection.yearId;
			selectedTermId = selection.termId;
			periods = [];
			entries = [];

			if (selection.replaceUrl) {
				await updateUrl(selection.replaceUrl);
				if (current !== revision) return;
			}

			if (!selectedYearId) {
				child = null;
				return;
			}

			const loadedChild = await getChildProfile(studentId, selectedYearId);
			if (current !== revision) return;
			child = loadedChild;
			if (selectedTermId) await loadTimetable(selectedTermId, current);
		} catch (error) {
			if (current === revision) {
				errorMessage = error instanceof Error ? error.message : 'โหลดข้อมูลตารางเรียนไม่สำเร็จ';
			}
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function loadTimetable(termId: string, current = ++revision): Promise<void> {
		const loaded = await getChildTimetable(studentId, termId);
		if (current !== revision) return;
		periods = periodsFromTimetableEntries(loaded);
		entries = loaded;
	}

	async function updateUrl(url: URL): Promise<void> {
		await goto(resolve(`/parent/student/${studentId}/timetable${url.search}${url.hash}`), {
			replaceState: true,
			noScroll: true,
			keepFocus: true
		});
	}

	async function changeYear(yearId: string): Promise<void> {
		if (yearId === selectedYearId) return;
		const current = ++revision;
		loading = true;
		errorMessage = '';
		const availableTerms =
			contextOptions?.terms.filter((term) => term.academicYearId === yearId) ?? [];
		const nextTerm =
			availableTerms.find((term) => term.id === contextOptions?.activeAcademicTermId) ??
			availableTerms[0];
		selectedYearId = yearId;
		selectedTermId = nextTerm?.id ?? '';
		periods = [];
		entries = [];
		try {
			const nextUrl = urlWithAcademicYear(page.url, selectedYearId);
			if (selectedTermId) nextUrl.searchParams.set('academicTermId', selectedTermId);
			await updateUrl(nextUrl);
			if (current !== revision) return;
			const loadedChild = await getChildProfile(studentId, selectedYearId);
			if (current !== revision) return;
			child = loadedChild;
			if (selectedTermId) await loadTimetable(selectedTermId, current);
		} catch (error) {
			if (current === revision) {
				errorMessage = error instanceof Error ? error.message : 'เปลี่ยนปีการศึกษาไม่สำเร็จ';
			}
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function changeTerm(termId: string): Promise<void> {
		const current = ++revision;
		selectedTermId = termId;
		periods = [];
		entries = [];
		loading = true;
		errorMessage = '';
		try {
			const nextUrl = new URL(page.url);
			nextUrl.searchParams.set('academicYearId', selectedYearId);
			nextUrl.searchParams.set('academicTermId', selectedTermId);
			await updateUrl(nextUrl);
			if (current !== revision) return;
			await loadTimetable(selectedTermId, current);
		} catch (error) {
			if (current === revision) {
				errorMessage = error instanceof Error ? error.message : 'โหลดตารางเรียนไม่สำเร็จ';
			}
		} finally {
			if (current === revision) loading = false;
		}
	}

	function entriesForCell(day: string, periodId: string): TimetableEntry[] {
		return entries.filter(
			(entry) => entry.dayOfWeek === day && entry.bellSchedulePeriodId === periodId
		);
	}

	function entryTitle(entry: TimetableEntry): string {
		return (
			entry.offeringCode ??
			entry.title ??
			entry.subjectVersionDisplayLabel ??
			entry.activityVersionDisplayLabel ??
			entry.entryType
		);
	}

	function entryColor(entryType: string): string {
		if (entryType === 'COURSE')
			return 'border-blue-200 bg-blue-50 text-blue-950 dark:border-blue-800 dark:bg-blue-950/40 dark:text-blue-100';
		if (entryType === 'ACTIVITY')
			return 'border-emerald-200 bg-emerald-50 text-emerald-950 dark:border-emerald-800 dark:bg-emerald-950/40 dark:text-emerald-100';
		if (entryType === 'BREAK')
			return 'border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-800 dark:bg-amber-950/40 dark:text-amber-100';
		return 'border-violet-200 bg-violet-50 text-violet-950 dark:border-violet-800 dark:bg-violet-950/40 dark:text-violet-100';
	}

	onMount(loadHistory);
</script>

<PageShell
	title="ตารางเรียน"
	description={childName ? `ตารางเรียนของ ${childName}` : 'ดูตารางเรียนย้อนหลังของนักเรียน'}
	backHref={childDetailHref}
>
	{#if contextOptions && contextOptions.years.length > 0}
		<div class="flex flex-wrap gap-3 rounded-xl border bg-card p-4">
			<div class="min-w-52 space-y-2">
				<Label for="parent-child-year">ปีการศึกษา</Label>
				<ScopedAcademicYearSelect
					id="parent-child-year"
					years={contextOptions.years}
					value={selectedYearId}
					disabled={loading}
					onchange={changeYear}
				/>
			</div>
			<div class="min-w-52 space-y-2">
				<Label for="parent-child-term">ภาคเรียน</Label>
				<Select.Root
					type="single"
					value={selectedTermId}
					disabled={loading || termOptions.length === 0}
					onValueChange={changeTerm}
				>
					<Select.Trigger id="parent-child-term" class="w-full">
						{termOptions.find((term) => term.id === selectedTermId)?.name ?? 'เลือกภาคเรียน'}
					</Select.Trigger>
					<Select.Content>
						{#each termOptions as term (term.id)}
							<Select.Item value={term.id}>{term.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
		</div>
	{/if}

	{#if loading}
		<PageSkeleton variant="table" rows={6} columns={Math.max(periods.length + 1, 4)} />
	{:else if errorMessage}
		<PageState
			variant="error"
			title="โหลดตารางเรียนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadHistory}
		/>
	{:else if !contextOptions || contextOptions.years.length === 0}
		<PageState
			title="ยังไม่มีประวัติปีการศึกษา"
			description="เมื่อโรงเรียนสร้างข้อมูลนักเรียนประจำปีแล้ว ประวัติจะปรากฏที่นี่"
		/>
	{:else if termOptions.length === 0}
		<PageState
			title="ยังไม่มีภาคเรียนในปีที่เลือก"
			description="โรงเรียนยังไม่ได้ตั้งค่าภาคเรียนสำหรับปีการศึกษานี้"
		/>
	{:else if entries.length === 0}
		<PageState
			title="ยังไม่มีตารางเรียน"
			description="โรงเรียนยังไม่ได้จัดตารางเรียนในภาคเรียนที่เลือก"
		/>
	{:else}
		<div class="overflow-x-auto rounded-lg border">
			<table class="w-full table-fixed border-collapse" style={`min-width: ${tableMinWidth}px`}>
				<thead>
					<tr>
						<th class="bg-muted/70 w-24 border p-2 text-xs">วัน / คาบ</th>
						{#each periods as period, index (period.id)}
							<th class="bg-muted/70 border p-2 text-center text-xs">
								<p class="font-semibold">{period.name ?? `คาบ ${index + 1}`}</p>
								<p class="text-muted-foreground font-normal">
									{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
								</p>
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each schoolDays as day (day.value)}
						<tr>
							<th class="bg-muted/30 border p-2 text-xs">{day.label}</th>
							{#each periods as period (period.id)}
								{@const cellEntries = entriesForCell(day.value, period.id)}
								<td class="h-24 border p-1 align-top">
									{#each cellEntries as entry (entry.id)}
										<div
											class={`mb-1 flex min-h-20 flex-col rounded-md border p-2 text-xs ${entryColor(entry.entryType)}`}
										>
											<p class="truncate font-semibold">{entryTitle(entry)}</p>
											{#if entry.offeringName}
												<p class="mt-1 line-clamp-2 opacity-80">{entry.offeringName}</p>
											{/if}
											{#if entry.learningGroupName || entry.homeroomName}
												<p class="mt-auto flex items-center gap-1 truncate opacity-70">
													<School class="size-3" />
													{entry.learningGroupName ?? entry.homeroomName}
												</p>
											{/if}
											{#if entry.roomCode}
												<p class="flex items-center gap-1 truncate opacity-70">
													<MapPin class="size-3" />
													{entry.roomCode}
												</p>
											{/if}
										</div>
									{/each}
								</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</PageShell>
