<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onDestroy, untrack } from 'svelte';
	import { addMonths } from 'date-fns';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import {
		listMyCalendarEvents,
		listChildCalendarEvents,
		type CalendarViewerEvent
	} from '#lib/api/calendar.js';
	import { calendarRouteFilters } from '#lib/utils/calendar-route-filters.js';
	import {
		calendarGridRange,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '#lib/utils/calendar.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { ChevronLeft, ChevronRight } from '@lucide/svelte';
	import CalendarEventList from './CalendarEventList.svelte';
	import CalendarMonthGrid from './CalendarMonthGrid.svelte';
	let {
		title,
		requestHref,
		requestKey,
		month: loadedMonth,
		records: source,
		studentId = null
	}: {
		title: string;
		requestHref: string;
		requestKey: string;
		month: string;
		studentId?: string | null;
		records: Promise<RouteLoadResult<{ ownerKey: string; records: CalendarViewerEvent[] }>>;
	} = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const allowed = $derived($authStore.user?.user_type === (studentId ? 'parent' : 'student'));
	const url = $derived.by(() => {
		const incoming = new URL(page.state.learnerCalendarUrl ?? requestHref);
		return incoming.pathname === new URL(requestHref).pathname ? incoming : new URL(requestHref);
	});
	const month = $derived(calendarRouteFilters(url).month);
	const key = $derived(`${identity}|${new URL(requestHref).pathname}|${month}`);
	let owner = '',
		disposed = false,
		consumed: typeof source | null = null;
	const read = new LatestRequest();
	let events = $state.raw<CalendarViewerEvent[]>([]),
		loading = $state(true),
		loaded = $state(false),
		error = $state(''),
		selectedDate = $state('');
	const selectedDateEvents = $derived(
		events
			.filter((event) => eventOverlapsDate(event, selectedDate))
			.sort(
				(left, right) =>
					left.startDate.localeCompare(right.startDate) ||
					(left.startTime ?? '').localeCompare(right.startTime ?? '') ||
					left.title.localeCompare(right.title, 'th')
			)
	);
	function current(k: string) {
		return !disposed && allowed && k === key;
	}
	function apply(result: Awaited<typeof source>, revision: number, k: string) {
		if (!current(k) || !read.isCurrent(revision)) return;
		loading = false;
		if (!result.ok) {
			error = result.error;
			return;
		}
		if (result.data.ownerKey !== k) return;
		events = result.data.records;
		loaded = true;
		error = '';
	}
	$effect.pre(() => {
		const k = key,
			operation = source,
			canRead = allowed,
			m = month;
		untrack(() => {
			const changed = owner !== k;
			if (changed || !canRead) {
				owner = k;
				read.abort();
				events = [];
				loaded = false;
				loading = canRead;
				error = '';
				selectedDate = m;
			}
			if (!canRead) return;
			if (operation !== consumed && m === loadedMonth && k === `${identity}|${requestKey}`) {
				consumed = operation;
				const ticket = read.begin();
				loading = true;
				void operation.then((result) => apply(result, ticket.revision, k));
			} else if (changed) {
				void reload();
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		read.abort();
	});
	async function reload() {
		if (!allowed) return;
		const k = key,
			ticket = read.begin();
		loading = true;
		error = '';
		const query = calendarGridRange(month);
		apply(
			await captureRouteLoad(
				(studentId
					? listChildCalendarEvents(studentId, query, { signal: ticket.signal })
					: listMyCalendarEvents(query, { signal: ticket.signal })
				).then((records) => ({ ownerKey: k, records })),
				'โหลดปฏิทินไม่สำเร็จ'
			),
			ticket.revision,
			k
		);
	}
	function changeMonth(offset: number) {
		const target = new URL(url);
		target.searchParams.delete('academicYearId');
		target.searchParams.delete('academicTermId');
		target.searchParams.set(
			'month',
			toIsoDate(addMonths(new Date(`${monthRange(month).from}T00:00:00`), offset)).slice(0, 7)
		);
		const path = studentId
			? resolve(`parent/student/${studentId}/calendar`)
			: resolve('student/calendar');
		void goto(path + target.search, {
			shallow: true,
			state: { ...page.state, learnerCalendarUrl: target.href }
		});
	}
</script>

<PageShell {title} description="กิจกรรมที่เกี่ยวข้องตามวันที่ในปฏิทิน">
	<div
		class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card p-3 sm:p-4"
	>
		<div class="flex items-center gap-3">
			<Button
				variant="outline"
				size="icon"
				onclick={() => changeMonth(-1)}
				aria-label="เดือนก่อนหน้า"><ChevronLeft class="size-4" /></Button
			>
			<h2 class="font-semibold">{formatCalendarMonth(month)}</h2>
			<Button variant="outline" size="icon" onclick={() => changeMonth(1)} aria-label="เดือนถัดไป"
				><ChevronRight class="size-4" /></Button
			>
		</div>
		<Button variant="ghost" onclick={reload} disabled={loading}>รีเฟรช</Button>
	</div>
	{#if !allowed}<PageState variant="permission" title="ไม่มีสิทธิ์ดูปฏิทินนี้" />{:else}
		<section
			data-testid={studentId ? 'parent-calendar-region' : 'student-calendar-region'}
			aria-busy={loading}
		>
			{#if error}<PageState
					variant="error"
					title="โหลดปฏิทินไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={reload}
				/>{/if}
			{#if loading && !loaded}<div role="status" aria-label={`กำลังโหลด${title}`}>
					<PageSkeleton variant="detail" />
				</div>{:else if loaded}
				{#if loading}<p
						role="status"
						aria-label="กำลังอัปเดตข้อมูล"
						class="text-sm text-muted-foreground"
					>
						กำลังอัปเดตกิจกรรม...
					</p>{/if}
				<div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_380px]">
					<CalendarMonthGrid
						monthDate={month}
						{events}
						{selectedDate}
						onselect={(date) => (selectedDate = date)}
					/>
					<section class="space-y-3">
						<h2 class="text-lg font-semibold">กิจกรรมวันที่ {formatCalendarDate(selectedDate)}</h2>
						<p class="text-sm text-muted-foreground">
							{selectedDateEvents.length} รายการในวันที่เลือก
						</p>
						<CalendarEventList events={selectedDateEvents} canManage={false} />
					</section>
				</div>
			{/if}
		</section>
	{/if}
</PageShell>
