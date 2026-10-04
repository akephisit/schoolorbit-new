<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { addMonths } from 'date-fns';
	import type { PageProps } from './$types';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { resolveScopedAcademicContextUrl } from '#lib/academic-context/scoped-year.js';
	import {
		listChildAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '#lib/api/academic-context.js';
	import { calendarRouteFilters } from '#lib/utils/calendar-route-filters.js';
	import { type CalendarViewerEvent, listChildCalendarEvents } from '#lib/api/calendar.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import CalendarEventList from '#lib/components/calendar/CalendarEventList.svelte';
	import CalendarMonthGrid from '#lib/components/calendar/CalendarMonthGrid.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import {
		calendarGridRange,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '#lib/utils/calendar.js';
	import { ChevronLeft, ChevronRight } from '@lucide/svelte';

	let { data }: PageProps = $props();
	const studentId = $derived(data.studentId);

	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const ownerKey = $derived(`${identityKey}|${data.requestKey}`);
	const allowed = $derived($authStore.user?.user_type === 'parent');
	let contextOptions = $state.raw<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state(''),
		selectedTermId = $state('');
	const childDetailHref = $derived(
		`/parent/student/${encodeURIComponent(studentId)}?academicYearId=${encodeURIComponent(selectedYearId)}`
	);
	let events = $state.raw<CalendarViewerEvent[]>([]);
	let loading = $state(true),
		loaded = $state(false),
		error = $state(''),
		contextLoading = $state(true),
		contextError = $state('');
	let disposed = false,
		owner = '';
	const contextRequest = new LatestRequest(),
		primaryRequest = new LatestRequest();
	let consumedContext: typeof data.context | null = null,
		consumedRecords: typeof data.records | null = null;
	let selectedMonth = $state(''),
		selectedDate = $state('');
	const ALL_TERMS_VALUE = '__all_terms__';
	const desiredMonth = $derived.by(() => {
		const url = new URL(page.state.learnerCalendarUrl ?? data.requestHref);
		return url.pathname === new URL(data.requestHref).pathname
			? calendarRouteFilters(url).month
			: data.month;
	});
	const termOptions = $derived(
		contextOptions?.terms.filter((term) => term.academicYearId === selectedYearId) ?? []
	);
	const monthLabel = $derived(formatCalendarMonth(selectedMonth));
	const selectedDateEvents = $derived(
		events
			.filter((event) => eventOverlapsDate(event, selectedDate))
			.sort((left, right) => left.startDate.localeCompare(right.startDate))
	);
	$effect.pre(() => {
		const key = ownerKey,
			a = data.context,
			b = data.records,
			canRead = allowed;
		const month = desiredMonth;
		untrack(() => {
			if (owner !== key || !canRead) {
				owner = key;

				contextRequest.abort();
				primaryRequest.abort();
				contextOptions = null;
				selectedYearId = '';
				selectedTermId = '';
				events = [];
				loaded = false;
				loading = canRead;
				contextLoading = canRead;
				error = '';
				contextError = '';
				selectedMonth = month;
				selectedDate = month;
			}
			if (!canRead) return;
			if (selectedMonth !== month) {
				primaryRequest.abort();
				selectedMonth = month;
				selectedDate = month;
				events = [];
				loaded = false;
				if (selectedYearId) void loadPrimary();
			}
			if (a !== consumedContext) {
				consumedContext = a;
				const t = contextRequest.begin();
				contextLoading = true;
				void a.then((v) => applyContext(v, t.revision, key));
			}
			if (b !== consumedRecords) {
				consumedRecords = b;
				if (data.month !== selectedMonth) return;
				const t = primaryRequest.begin();
				loading = true;
				void b.then((v) => applyRecords(v, t.revision, key, data.month));
			}
		});
	});
	onDestroy(() => {
		disposed = true;

		contextRequest.abort();
		primaryRequest.abort();
	});
	function current(key: string) {
		return !disposed && allowed && key === ownerKey;
	}
	function applyContext(v: Awaited<typeof data.context>, revision: number, key: string) {
		if (!current(key) || !contextRequest.isCurrent(revision)) return;
		contextLoading = false;
		if (!v.ok) {
			contextError = v.error;
			return;
		}
		if (v.data.ownerKey !== key) return;
		contextError = '';
		contextOptions = v.data.options;
		selectedYearId = v.data.academicYearId;
		selectedTermId = v.data.academicTermId;
		if (v.data.replaceHref) {
			const url = new URL(v.data.replaceHref);

			goto(resolve(`parent/student/${page.params.id}/calendar`) + url.search, {
				shallow: true,
				replace: true,
				state: page.state
			});
		}
	}
	function applyRecords(
		v: Awaited<typeof data.records>,
		revision: number,
		key: string,
		requestedMonth: string
	) {
		if (!current(key) || !primaryRequest.isCurrent(revision) || requestedMonth !== selectedMonth)
			return;
		loading = false;
		if (!v.ok) {
			error = v.error;
			return;
		}
		if (v.data.ownerKey !== key) return;
		events = v.data.records;
		loaded = contextOptions !== null;
		error = '';
	}
	async function loadPrimary() {
		if (!allowed || disposed || !selectedYearId) return;
		const key = ownerKey,
			t = primaryRequest.begin();
		const requestedMonth = selectedMonth;
		loading = true;
		error = '';
		const v = await captureRouteLoad(
			listChildCalendarEvents(
				studentId,
				{
					academicYearId: selectedYearId,
					academicTermId: selectedTermId || undefined,
					...calendarGridRange(selectedMonth)
				},
				{ signal: t.signal }
			).then((records) => ({ ownerKey: key, records })),
			'โหลดปฏิทินไม่สำเร็จ'
		);
		applyRecords(v, t.revision, key, requestedMonth);
	}
	async function retryContext() {
		if (!allowed || disposed) return;
		const key = ownerKey,
			t = contextRequest.begin();
		contextLoading = true;
		contextError = '';
		const v = await captureRouteLoad(
			listChildAcademicContextOptions(studentId, t.signal).then((options) => {
				const selection = resolveScopedAcademicContextUrl(
					options,
					new URL(data.requestHref),
					false
				);
				return {
					ownerKey: key,
					options,
					academicYearId: selection.academicYearId,
					academicTermId: selection.academicTermId,
					replaceHref: selection.replaceUrl?.href ?? null
				};
			}),
			'โหลดประวัติปีและภาคเรียนไม่สำเร็จ'
		);
		if (!current(key) || !contextRequest.isCurrent(t.revision)) return;
		applyContext(v, t.revision, key);
		if (v.ok && selectedYearId) await loadPrimary();
	}
	async function updateUrl(yearId: string, termId: string) {
		const url = new URL(data.requestHref);
		url.searchParams.set('academicYearId', yearId);
		if (termId) url.searchParams.set('academicTermId', termId);
		else url.searchParams.delete('academicTermId');
		url.searchParams.set('month', selectedMonth.slice(0, 7));
		await goto(resolve(`parent/student/${page.params.id}/calendar`) + url.search, {
			reset: false
		});
	}
	async function changeYear(yearId: string) {
		if (!contextOptions?.years.some((year) => year.id === yearId) || yearId === selectedYearId)
			return;
		const next = '';
		await updateUrl(yearId, next);
	}
	async function changeTerm(value: string) {
		const termId = value === ALL_TERMS_VALUE ? '' : value;
		if (termId && !termOptions.some((term) => term.id === termId)) return;
		if (termId === selectedTermId) return;
		await updateUrl(selectedYearId, termId);
	}
	function changeMonth(offset: number) {
		if (contextLoading || !selectedYearId) return;
		const next = monthRange(
			toIsoDate(addMonths(new Date(`${monthRange(selectedMonth).from}T00:00:00`), offset))
		).from;
		const url = new URL(data.requestHref);
		url.searchParams.set('academicYearId', selectedYearId);
		if (selectedTermId) url.searchParams.set('academicTermId', selectedTermId);
		else url.searchParams.delete('academicTermId');
		url.searchParams.set('month', next.slice(0, 7));
		goto(resolve(`parent/student/${page.params.id}/calendar`) + url.search, {
			shallow: true,
			state: { ...page.state, learnerCalendarUrl: url.href }
		});
	}
</script>

<PageShell
	backHref={childDetailHref}
	title="ปฏิทินของลูก"
	description="กิจกรรมที่เกี่ยวข้องกับนักเรียน"
>
	<div class="flex flex-wrap gap-3 rounded-xl border bg-card p-4">
		<div class="min-w-52 space-y-2">
			<Label for="student-calendar-year">ปีการศึกษา</Label>
			<Select.Root
				type="single"
				value={selectedYearId}
				disabled={contextLoading}
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
				disabled={contextLoading || !selectedYearId}
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
		<Button variant="ghost" onclick={loadPrimary}>รีเฟรช</Button>
	</div>

	{#if contextError}<PageState
			variant="error"
			title="โหลดประวัติปีและภาคเรียนไม่สำเร็จ"
			description={contextError}
			actionLabel="ลองบริบทอีกครั้ง"
			onaction={retryContext}
		/>{/if}
	{#if error}
		<PageState
			variant="error"
			title="โหลดปฏิทินไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadPrimary}
		/>
	{/if}
	<div data-testid="parent-calendar-region" aria-busy={loading || contextLoading}>
		{#if loading && loaded}
			<p role="status" aria-label="กำลังอัปเดตข้อมูล" class="text-muted-foreground text-sm">
				กำลังอัปเดตข้อมูล…
			</p>
		{/if}
		{#if contextLoading || (loading && !loaded)}
			<div role="status" aria-label="กำลังโหลดปฏิทิน">
				<PageSkeleton variant="detail" />
			</div>
		{:else if contextOptions && contextOptions.years.length === 0 && !contextError}
			<PageState
				title="ยังไม่มีประวัติปีการศึกษา"
				description="เมื่อโรงเรียนสร้างข้อมูลนักเรียนประจำปีแล้ว ปฏิทินจะปรากฏที่นี่"
			/>
		{:else if contextOptions && !contextError}
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
	</div>
</PageShell>
