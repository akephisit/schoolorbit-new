<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { addMonths } from 'date-fns';
	import { onMount } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import {
		listMyAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import { type CalendarViewerEvent, listMyCalendarEvents } from '$lib/api/calendar';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import CalendarEventList from '$lib/components/calendar/CalendarEventList.svelte';
	import CalendarMonthGrid from '$lib/components/calendar/CalendarMonthGrid.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import {
		calendarGridRange,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '$lib/utils/calendar';
	import { ChevronLeft, ChevronRight } from 'lucide-svelte';

	let events = $state.raw<CalendarViewerEvent[]>([]);
	let loading = $state(true);
	let error = $state('');
	let selectedMonth = $state(toIsoDate(new Date()));
	let selectedDate = $state(toIsoDate(new Date()));
	let contextOptions = $state<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let selectedTermId = $state('');
	let requestToken = 0;
	const ALL_TERMS_VALUE = '__all_terms__';

	const monthLabel = $derived(formatCalendarMonth(selectedMonth));
	const termOptions = $derived(
		contextOptions?.terms.filter((term) => term.academicYearId === selectedYearId) ?? []
	);
	const selectedDateEvents = $derived(
		events
			.filter((event) => eventOverlapsDate(event, selectedDate))
			.sort((left, right) => left.startDate.localeCompare(right.startDate))
	);

	function authorizedSelection(options: AcademicContextOptionsResponse) {
		const requestedYearId = page.url.searchParams.get('academicYearId');
		const yearId =
			options.years.find((year) => year.id === requestedYearId)?.id ??
			options.years.find((year) => year.id === options.activeAcademicYearId)?.id ??
			options.years[0]?.id ??
			'';
		const requestedTermId = page.url.searchParams.get('academicTermId');
		const termId =
			options.terms.find((term) => term.id === requestedTermId && term.academicYearId === yearId)
				?.id ?? '';
		return { yearId, termId };
	}

	async function loadCalendar() {
		const currentRequest = ++requestToken;
		loading = true;
		error = '';
		try {
			if (!selectedYearId) {
				events = [];
				return;
			}
			const nextEvents = await listMyCalendarEvents({
				academicYearId: selectedYearId,
				academicTermId: selectedTermId || undefined,
				...calendarGridRange(selectedMonth)
			});
			if (currentRequest === requestToken) events = nextEvents;
		} catch (loadError: unknown) {
			if (currentRequest === requestToken) {
				error =
					(loadError instanceof Error ? loadError.message : String(loadError)) ||
					'โหลดปฏิทินไม่สำเร็จ';
				toast.error(error);
			}
		} finally {
			if (currentRequest === requestToken) loading = false;
		}
	}

	async function loadHistory() {
		const currentRequest = ++requestToken;
		loading = true;
		error = '';
		try {
			const options = await listMyAcademicContextOptions();
			if (currentRequest !== requestToken) return;
			contextOptions = options;
			const selection = authorizedSelection(options);
			selectedYearId = selection.yearId;
			selectedTermId = selection.termId;
			if (!selectedYearId) {
				events = [];
				return;
			}
			const nextEvents = await listMyCalendarEvents({
				academicYearId: selectedYearId,
				academicTermId: selectedTermId || undefined,
				...calendarGridRange(selectedMonth)
			});
			if (currentRequest === requestToken) events = nextEvents;
		} catch (loadError: unknown) {
			if (currentRequest === requestToken) {
				error = loadError instanceof Error ? loadError.message : 'โหลดปฏิทินไม่สำเร็จ';
				toast.error(error);
			}
		} finally {
			if (currentRequest === requestToken) loading = false;
		}
	}

	async function updateUrl(yearId: string, termId: string) {
		const query = new SvelteURLSearchParams({ academicYearId: yearId });
		if (termId) query.set('academicTermId', termId);
		await goto(resolve(`/student/calendar?${query.toString()}`), {
			noScroll: true,
			keepFocus: true
		});
	}

	async function changeYear(value: string) {
		selectedYearId = value;
		selectedTermId = '';
		events = [];
		await updateUrl(selectedYearId, selectedTermId);
		await loadCalendar();
	}

	async function changeTerm(value: string) {
		selectedTermId = value === ALL_TERMS_VALUE ? '' : value;
		await updateUrl(selectedYearId, selectedTermId);
		await loadCalendar();
	}

	async function changeMonth(offset: number) {
		const currentMonthStart = monthRange(selectedMonth).from;
		const nextMonth = monthRange(
			toIsoDate(addMonths(new Date(`${currentMonthStart}T00:00:00`), offset))
		).from;
		selectedMonth = nextMonth;
		selectedDate = nextMonth;
		await loadCalendar();
	}

	onMount(loadHistory);
</script>

<PageShell title="ปฏิทิน" description="กิจกรรมที่เกี่ยวข้องกับคุณ">
	<div class="flex flex-wrap gap-3 rounded-xl border bg-card p-4">
		<div class="min-w-52 space-y-2">
			<Label for="student-calendar-year">ปีการศึกษา</Label>
			<Select.Root
				type="single"
				value={selectedYearId}
				disabled={loading}
				onValueChange={(value) => void changeYear(value)}
			>
				<Select.Trigger id="student-calendar-year" class="w-full">
					{contextOptions?.years.find((year) => year.id === selectedYearId)?.name ??
						'เลือกปีการศึกษา'}
				</Select.Trigger>
				<Select.Content>
					{#each contextOptions?.years ?? [] as year (year.id)}
						<Select.Item value={year.id}>{year.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
		<div class="min-w-52 space-y-2">
			<Label for="student-calendar-term">ภาคเรียน</Label>
			<Select.Root
				type="single"
				value={selectedTermId || ALL_TERMS_VALUE}
				disabled={loading || !selectedYearId}
				onValueChange={(value) => void changeTerm(value)}
			>
				<Select.Trigger id="student-calendar-term" class="w-full">
					{termOptions.find((term) => term.id === selectedTermId)?.name ?? 'ทั้งปี'}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value={ALL_TERMS_VALUE}>ทั้งปี</Select.Item>
					{#each termOptions as term (term.id)}
						<Select.Item value={term.id}>{term.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
	</div>

	<div
		class="flex flex-wrap items-center justify-between gap-3 rounded-md border bg-background p-4"
	>
		<div class="flex items-center gap-2">
			<Button
				variant="outline"
				size="icon"
				onclick={() => changeMonth(-1)}
				aria-label="เดือนก่อนหน้า"
			>
				<ChevronLeft class="h-4 w-4" />
			</Button>
			<div class="min-w-44 text-center text-sm font-medium">{monthLabel}</div>
			<Button variant="outline" size="icon" onclick={() => changeMonth(1)} aria-label="เดือนถัดไป">
				<ChevronRight class="h-4 w-4" />
			</Button>
		</div>
		<Button variant="ghost" onclick={loadCalendar}>รีเฟรช</Button>
	</div>

	{#if loading}
		<PageSkeleton variant="detail" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดปฏิทินไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadHistory}
		/>
	{:else if !contextOptions || contextOptions.years.length === 0}
		<PageState
			title="ยังไม่มีประวัติปีการศึกษา"
			description="เมื่อโรงเรียนสร้างข้อมูลนักเรียนประจำปีแล้ว ปฏิทินจะปรากฏที่นี่"
		/>
	{:else}
		<div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_380px]">
			<CalendarMonthGrid
				monthDate={selectedMonth}
				{events}
				{selectedDate}
				onselect={(date) => (selectedDate = date)}
			/>
			<section class="space-y-3">
				<div>
					<h2 class="text-lg font-semibold">กิจกรรมวันที่ {formatCalendarDate(selectedDate)}</h2>
					<p class="text-sm text-muted-foreground">
						{selectedDateEvents.length} รายการในวันที่เลือก
					</p>
				</div>
				<CalendarEventList events={selectedDateEvents} canManage={false} />
			</section>
		</div>
	{/if}
</PageShell>
