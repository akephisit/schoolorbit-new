<script lang="ts">
	import { onMount } from 'svelte';
	import { addMonths } from 'date-fns';
	import {
		listPublicAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import CalendarColorKey from '$lib/components/calendar/CalendarColorKey.svelte';
	import CalendarDayTimelineDialog from '$lib/components/calendar/CalendarDayTimelineDialog.svelte';
	import CalendarMonthGrid from '$lib/components/calendar/CalendarMonthGrid.svelte';
	import CalendarEventList from '$lib/components/calendar/CalendarEventList.svelte';
	import { type CalendarPublicEvent, listPublicCalendarEvents } from '$lib/api/calendar';
	import {
		buildCalendarColorKey,
		calendarGridRange,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '$lib/utils/calendar';
	import { CalendarDays, ChevronLeft, ChevronRight } from 'lucide-svelte';

	type PublicCalendarMode = 'page' | 'embed';

	let { mode = 'page' }: { mode?: PublicCalendarMode } = $props();

	let events = $state.raw<CalendarPublicEvent[]>([]);
	let loading = $state(true);
	let error = $state('');
	let selectedMonth = $state(toIsoDate(new Date()));
	let selectedDate = $state(toIsoDate(new Date()));
	let dayDialogOpen = $state(false);
	let contextOptions = $state<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let selectedTermId = $state('');
	let requestToken = 0;

	const embedded = $derived(mode === 'embed');
	const termOptions = $derived(
		contextOptions?.terms.filter((term) => term.academicYearId === selectedYearId) ?? []
	);
	const monthLabel = $derived(formatCalendarMonth(selectedMonth));
	const colorKeyItems = $derived(buildCalendarColorKey(selectedMonth, events));
	const selectedDateEvents = $derived(
		events
			.filter((event) => eventOverlapsDate(event, selectedDate))
			.sort(
				(left, right) =>
					left.startDate.localeCompare(right.startDate) ||
					Number(right.allDay) - Number(left.allDay) ||
					(left.startTime ?? '').localeCompare(right.startTime ?? '') ||
					left.title.localeCompare(right.title, 'th')
			)
	);

	async function loadCalendar() {
		const currentRequest = ++requestToken;
		loading = true;
		error = '';
		try {
			if (!selectedYearId) {
				events = [];
				return;
			}
			const nextEvents = await listPublicCalendarEvents({
				academicYearId: selectedYearId,
				academicTermId: selectedTermId || null,
				...calendarGridRange(selectedMonth)
			});
			if (currentRequest === requestToken) events = nextEvents;
		} catch (loadError: unknown) {
			if (currentRequest === requestToken) {
				error =
					(loadError instanceof Error ? loadError.message : String(loadError)) ||
					'โหลดปฏิทินไม่สำเร็จ';
			}
		} finally {
			if (currentRequest === requestToken) loading = false;
		}
	}

	async function loadContext() {
		const currentRequest = ++requestToken;
		loading = true;
		error = '';
		try {
			const options = await listPublicAcademicContextOptions();
			if (currentRequest !== requestToken) return;
			contextOptions = options;
			selectedYearId =
				options.years.find((year) => year.id === options.activeAcademicYearId)?.id ??
				options.years[0]?.id ??
				'';
			selectedTermId = '';
			if (!selectedYearId) {
				events = [];
				return;
			}
			const nextEvents = await listPublicCalendarEvents({
				academicYearId: selectedYearId,
				academicTermId: null,
				...calendarGridRange(selectedMonth)
			});
			if (currentRequest === requestToken) events = nextEvents;
		} catch (loadError: unknown) {
			if (currentRequest === requestToken) {
				error = loadError instanceof Error ? loadError.message : 'โหลดปฏิทินไม่สำเร็จ';
			}
		} finally {
			if (currentRequest === requestToken) loading = false;
		}
	}

	async function changeYear(event: Event) {
		selectedYearId = (event.currentTarget as HTMLSelectElement).value;
		selectedTermId = '';
		await loadCalendar();
	}

	async function changeTerm(event: Event) {
		selectedTermId = (event.currentTarget as HTMLSelectElement).value;
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

	async function goToToday() {
		const today = toIsoDate(new Date());
		selectedMonth = monthRange(today).from;
		selectedDate = today;
		await loadCalendar();
	}

	function selectDate(date: string) {
		selectedDate = date;
		if (window.matchMedia('(max-width: 1023px)').matches) {
			dayDialogOpen = true;
		}
	}

	onMount(loadContext);
</script>

<main
	class={embedded ? 'h-dvh overflow-hidden bg-background' : 'h-dvh overflow-hidden bg-muted/20'}
>
	<section
		data-calendar-mode={mode}
		class={embedded
			? 'flex h-full w-full flex-col gap-2 p-2 sm:gap-3 sm:p-3'
			: 'mx-auto flex h-full w-full max-w-screen-2xl flex-col gap-3 px-3 py-3 sm:px-4 lg:gap-4 lg:px-8 lg:py-4 2xl:px-10'}
	>
		<header
			class={embedded
				? 'flex shrink-0 items-center justify-between gap-2 border-b pb-2'
				: 'flex shrink-0 flex-col gap-2 border-b pb-3 sm:flex-row sm:items-end sm:justify-between'}
		>
			{#if !embedded}
				<div class="flex items-center gap-3">
					<div
						class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary sm:size-10"
					>
						<CalendarDays class="size-5" />
					</div>
					<div class="min-w-0">
						<h1 class="text-lg font-semibold tracking-tight sm:text-2xl">ปฏิทินโรงเรียน</h1>
						<p class="hidden text-sm text-muted-foreground sm:block">
							กิจกรรมที่โรงเรียนเปิดเผยต่อสาธารณะ
						</p>
					</div>
				</div>
			{/if}

			<div
				class={embedded
					? 'flex w-full flex-wrap items-center justify-between gap-2'
					: 'flex flex-wrap items-center justify-between gap-2 sm:justify-end'}
			>
				<div class="flex items-end gap-2">
					<div class="space-y-1">
						<Label for={`public-calendar-year-${mode}`} class="text-xs">ปีการศึกษา</Label>
						<select
							id={`public-calendar-year-${mode}`}
							class="border-input bg-background h-8 max-w-36 rounded-md border px-2 text-xs"
							value={selectedYearId}
							disabled={loading}
							onchange={changeYear}
						>
							{#each contextOptions?.years ?? [] as year (year.id)}
								<option value={year.id}>{year.name}</option>
							{/each}
						</select>
					</div>
					<div class="space-y-1">
						<Label for={`public-calendar-term-${mode}`} class="text-xs">ภาคเรียน</Label>
						<select
							id={`public-calendar-term-${mode}`}
							class="border-input bg-background h-8 max-w-36 rounded-md border px-2 text-xs"
							value={selectedTermId}
							disabled={loading || !selectedYearId}
							onchange={changeTerm}
						>
							<option value="">ทั้งปี</option>
							{#each termOptions as term (term.id)}
								<option value={term.id}>{term.name}</option>
							{/each}
						</select>
					</div>
				</div>
				<Button variant="outline" size="sm" onclick={goToToday}>วันนี้</Button>
				<div class="flex items-center gap-1 sm:gap-2">
					<Button
						variant="outline"
						size="icon-sm"
						onclick={() => changeMonth(-1)}
						aria-label="เดือนก่อนหน้า"
					>
						<ChevronLeft class="h-4 w-4" />
					</Button>
					<div class="min-w-32 text-center text-sm font-semibold sm:min-w-40">{monthLabel}</div>
					<Button
						variant="outline"
						size="icon-sm"
						onclick={() => changeMonth(1)}
						aria-label="เดือนถัดไป"
					>
						<ChevronRight class="h-4 w-4" />
					</Button>
				</div>
			</div>
		</header>

		{#if loading}
			<div class="min-h-0 flex-1 overflow-hidden">
				<PageSkeleton variant="detail" />
			</div>
		{:else if error}
			<div class="min-h-0 flex-1 overflow-y-auto">
				<PageState
					variant="error"
					title="โหลดปฏิทินไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={loadCalendar}
				/>
			</div>
		{:else if !contextOptions || contextOptions.years.length === 0}
			<div class="min-h-0 flex-1 overflow-y-auto">
				<PageState
					title="ยังไม่มีปีการศึกษาที่เผยแพร่"
					description="ปฏิทินจะพร้อมใช้งานเมื่อโรงเรียนเปิดปีการศึกษา"
				/>
			</div>
		{:else}
			<div
				class="grid min-h-0 flex-1 lg:grid-cols-[minmax(0,1fr)_22rem] lg:gap-5 xl:grid-cols-[minmax(0,1fr)_24rem]"
			>
				<div class="flex min-h-0 min-w-0 flex-col gap-3">
					<div class="min-h-0 flex-1">
						<CalendarMonthGrid
							monthDate={selectedMonth}
							{events}
							{selectedDate}
							onselect={selectDate}
							fillHeight
						/>
					</div>
					{#if colorKeyItems.length > 0}
						<CalendarColorKey items={colorKeyItems} />
					{/if}
				</div>
				<aside
					class="hidden min-h-0 flex-col overflow-hidden rounded-xl border bg-card shadow-sm lg:flex"
				>
					<div class="flex shrink-0 items-end justify-between gap-3 border-b px-3 py-2.5 sm:px-4">
						<div>
							<p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
								วันที่เลือก
							</p>
							<h2 class="mt-1 text-lg font-semibold">{formatCalendarDate(selectedDate)}</h2>
						</div>
						<span class="shrink-0 text-sm text-muted-foreground">
							{selectedDateEvents.length} รายการ
						</span>
					</div>
					<div class="min-h-0 flex-1 overflow-y-auto p-3 sm:p-4">
						<CalendarEventList events={selectedDateEvents} canManage={false} showFullDescription />
					</div>
				</aside>
			</div>
		{/if}
	</section>
</main>

<CalendarDayTimelineDialog
	bind:open={dayDialogOpen}
	date={selectedDate}
	events={selectedDateEvents}
/>
