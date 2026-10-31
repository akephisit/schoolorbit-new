<script lang="ts">
	import { untrack } from 'svelte';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
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
		initialQuery = '',
		filters = {},
		onselect
	}: {
		open: boolean;
		kind: 'staff' | 'public';
		initialQuery?: string;
		filters?: CalendarEventFilters;
		onselect: (event: { id: string; title: string; startDate: string }) => void;
	} = $props();
	let query = $state(''),
		offset = $state(0);
	let rows = $state.raw<(CalendarEvent | CalendarPublicEvent)[]>([]);
	let reading = $state(false),
		error = $state('');
	const request = new LatestRequest();
	const results = $derived(rows.slice(0, 100));
	const hasMore = $derived(rows.length > 100 && offset < 10_000);
	$effect.pre(() => {
		const opened = open;
		untrack(() => {
			request.abort();
			rows = [];
			error = '';
			reading = false;
			if (!opened) return;
			query = initialQuery;
			offset = 0;
		});
		return () => {
			request.abort();
		};
	});
	$effect(() => {
		const opened = open,
			term = query.trim(),
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
		reading = true;
		const timer = setTimeout(() => void searchEvents(term, pageOffset), 300);
		return () => {
			clearTimeout(timer);
			request.abort();
		};
	});
	async function searchEvents(term: string, pageOffset: number) {
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
			offset: pageOffset
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
								tagId: filters.tagId
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
			<div class="space-y-2" aria-busy={reading} aria-live="polite">
				{#if reading}<PageSkeleton variant="cards" rows={3} />
				{:else if error}<PageState
						variant="error"
						title="ค้นหากิจกรรมไม่สำเร็จ"
						description={error}
						actionLabel="ลองอีกครั้ง"
						onaction={() => void searchEvents(query.trim(), offset)}
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
										onselect(event);
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
