<script lang="ts">
	import { untrack } from 'svelte';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		listPublicAcademicContextOptions,
		type AcademicYearOption
	} from '#lib/api/academic-context.js';
	import {
		listCalendarEvents,
		listPublicCalendarEvents,
		type CalendarEvent,
		type CalendarPublicEvent,
		type CalendarEventFilters
	} from '#lib/api/calendar.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';
	import { Search, CalendarDays, MapPin } from '@lucide/svelte';

	let {
		open = $bindable(false),
		kind,
		yearOptions,
		yearId = '',
		initialQuery = '',
		filters = {},
		onselect
	}: {
		open: boolean;
		kind: 'staff' | 'public';
		yearOptions?: AcademicYearOption[];
		yearId?: string;
		initialQuery?: string;
		filters?: CalendarEventFilters;
		onselect: (event: { id: string; title: string; startDate: string }, yearId: string) => void;
	} = $props();
	let query = $state(''),
		selectedYearId = $state(''),
		offset = $state(0);
	let years = $state<AcademicYearOption[]>([]),
		rows = $state.raw<(CalendarEvent | CalendarPublicEvent)[]>([]);
	let reading = $state(false),
		yearsLoading = $state(false),
		error = $state(''),
		yearsError = $state('');
	const request = new LatestRequest(),
		contextRequest = new LatestRequest();
	const selectedYear = $derived(years.find((year) => year.id === selectedYearId));
	const results = $derived(rows.slice(0, 100));
	const hasMore = $derived(rows.length > 100 && offset < 10_000);
	$effect(() => {
		if (open && yearOptions !== undefined) years = yearOptions;
	});

	$effect.pre(() => {
		const opened = open;
		untrack(() => {
			request.abort();
			contextRequest.abort();
			rows = [];
			error = '';
			reading = false;
			yearsError = '';
			yearsLoading = false;
			if (!opened) return;
			query = initialQuery;
			selectedYearId = yearId;
			offset = 0;
			years = yearOptions ?? [];
			if (yearOptions === undefined) void loadYears();
		});
		return () => {
			request.abort();
			contextRequest.abort();
		};
	});
	async function loadYears() {
		const ticket = contextRequest.begin();
		yearsLoading = true;
		yearsError = '';
		try {
			const result = await listPublicAcademicContextOptions(ticket.signal);
			if (contextRequest.isCurrent(ticket.revision)) years = result.years;
		} catch (error) {
			if (!isAbortError(error) && contextRequest.isCurrent(ticket.revision))
				yearsError = error instanceof Error ? error.message : 'โหลดปีไม่สำเร็จ';
		} finally {
			if (contextRequest.isCurrent(ticket.revision)) yearsLoading = false;
		}
	}
	$effect(() => {
		const opened = open,
			term = query.trim(),
			year = selectedYear,
			id = selectedYearId,
			pageOffset = offset;
		request.abort();
		untrack(() => {
			rows = [];
			error = '';
			reading = false;
		});
		if (!opened || !term) return;
		if (term.length > 200) {
			error = 'คำค้นหายาวเกิน 200 ตัวอักษร';
			return;
		}
		if (id && !year) return;
		reading = true;
		const timer = setTimeout(() => void searchEvents(term, year, pageOffset), 300);
		return () => {
			clearTimeout(timer);
			request.abort();
		};
	});
	async function searchEvents(
		term: string,
		year: AcademicYearOption | undefined,
		pageOffset: number
	) {
		const ticket = request.begin();
		reading = true;
		error = '';
		const searchFilters: CalendarEventFilters = {
			categoryId: filters.categoryId,
			tagId: filters.tagId,
			audience: filters.audience,
			visibility: filters.visibility,
			search: true,
			q: term,
			offset: pageOffset,
			...(year ? { from: year.startDate, to: year.endDate } : {})
		};
		try {
			const result =
				kind === 'staff'
					? await listCalendarEvents(searchFilters, { signal: ticket.signal })
					: await listPublicCalendarEvents(
							{
								search: true,
								q: term,
								offset: pageOffset,
								categoryId: filters.categoryId,
								tagId: filters.tagId,
								...(year ? { from: year.startDate, to: year.endDate } : {})
							},
							{ signal: ticket.signal }
						);
			if (request.isCurrent(ticket.revision)) rows = result;
		} catch (failure) {
			if (!isAbortError(failure) && request.isCurrent(ticket.revision))
				error = failure instanceof Error ? failure.message : 'ค้นหากิจกรรมไม่สำเร็จ';
		} finally {
			if (request.isCurrent(ticket.revision)) reading = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header
			><Dialog.Title>ค้นหากิจกรรม</Dialog.Title><Dialog.Description
				>พิมพ์บางส่วนของชื่อ รายละเอียด สถานที่ หรือแท็ก เพื่อค้นหาข้ามเดือน</Dialog.Description
			></Dialog.Header
		>
		<div class="space-y-3">
			<div class="space-y-1.5">
				<Label for="calendar-search-text">คำค้นหากิจกรรม</Label><Input
					id="calendar-search-text"
					bind:value={query}
					oninput={() => (offset = 0)}
					placeholder="เช่น ทัศน หรือ ประชุม"
					maxlength={200}
				/>
			</div>
			<div class="space-y-1.5">
				<Label for="calendar-search-year">ปีการศึกษาที่ค้นหา</Label>
				<Select.Root
					type="single"
					value={selectedYearId}
					onValueChange={(value) => {
						selectedYearId = value;
						offset = 0;
					}}
				>
					<Select.Trigger id="calendar-search-year" class="w-full" disabled={yearsLoading}
						>{selectedYear?.name ?? 'ทุกปี / ทุกช่วงเวลา'}</Select.Trigger
					>
					<Select.Content
						><Select.Item value="">ทุกปี / ทุกช่วงเวลา</Select.Item
						>{#each years as year (year.id)}<Select.Item value={year.id}>{year.name}</Select.Item
							>{/each}</Select.Content
					>
				</Select.Root>
				{#if selectedYear}<p class="text-xs text-muted-foreground">
						{formatCalendarDate(selectedYear.startDate)} – {formatCalendarDate(
							selectedYear.endDate
						)}
					</p>{/if}
			</div>
			{#if yearsError}<div
					role="alert"
					class="flex flex-wrap items-center gap-2 text-sm text-destructive"
				>
					<span>{yearsError}</span><Button size="sm" variant="outline" onclick={loadYears}
						>ลองโหลดปีอีกครั้ง</Button
					>
				</div>{/if}
			<div class="space-y-2" aria-busy={reading} aria-live="polite">
				{#if reading || (query.trim() && selectedYearId && yearsLoading)}<PageSkeleton
						variant="cards"
						rows={3}
					/>
				{:else if selectedYearId && !selectedYear}<PageState
						variant="error"
						title="ไม่พบปีการศึกษาที่เลือก"
						description="เลือกปีใหม่ หรือค้นหาทุกช่วงเวลา"
					/>
				{:else if error}<PageState
						variant="error"
						title="ค้นหากิจกรรมไม่สำเร็จ"
						description={error}
						actionLabel="ลองอีกครั้ง"
						onaction={() => void searchEvents(query.trim(), selectedYear, offset)}
					/>
				{:else if !query.trim()}<PageState
						title="เริ่มพิมพ์เพื่อค้นหากิจกรรม"
						description="ค้นหาได้แม้กิจกรรมอยู่คนละเดือนกับปฏิทินที่เปิด"
					/>
				{:else if results.length === 0}<PageState
						title="ไม่พบกิจกรรมที่ตรงกับคำค้นหา"
						description="ลองใช้คำที่สั้นลง หรือเลือกทุกปี / ทุกช่วงเวลา"
					/>
				{:else}
					<p class="text-xs text-muted-foreground">
						แสดงรายการที่ {offset + 1}–{offset + results.length}{#if hasMore}
							· ยังมีรายการเพิ่มเติม{/if}
					</p>
					<ul class="space-y-2">
						{#each results as event (event.id)}<li>
								<Button
									variant="outline"
									class="h-auto w-full justify-start whitespace-normal p-3 text-left"
									onclick={() => {
										onselect(event, selectedYearId);
										open = false;
									}}
								>
									<Search class="size-4 shrink-0 text-primary" /><span class="min-w-0 space-y-1"
										><span class="block font-medium">{event.title}</span><span
											class="flex items-center gap-1 text-xs text-muted-foreground"
											><CalendarDays class="size-3" />{formatCalendarDate(
												event.startDate
											)}{#if event.endDate !== event.startDate}
												– {formatCalendarDate(event.endDate)}{/if}</span
										>{#if event.location}<span
												class="flex items-center gap-1 text-xs text-muted-foreground"
												><MapPin class="size-3" />{event.location}</span
											>{/if}</span
									>
								</Button>
							</li>{/each}
					</ul>
				{/if}
			</div>
			{#if offset > 0 || hasMore}<div class="flex justify-between gap-2">
					<Button
						variant="outline"
						size="sm"
						disabled={reading || offset === 0}
						onclick={() => (offset = Math.max(0, offset - 100))}>หน้าก่อนหน้า</Button
					><Button
						variant="outline"
						size="sm"
						disabled={reading || !hasMore}
						onclick={() => (offset += 100)}>หน้าถัดไป</Button
					>
				</div>{/if}
		</div>
	</Dialog.Content>
</Dialog.Root>
