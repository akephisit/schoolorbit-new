<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { toast } from 'svelte-sonner';
	import { academicContextualMenuPath } from '#lib/academic-context/route-context.js';
	import {
		getDailyTeachingOverview,
		type DailyTeachingEntry,
		type DailyTeachingOverview,
		type DailyTeachingPeriod
	} from '#lib/api/timetable.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import TimetableOwnerSelector from '#lib/components/academic/timetable/TimetableOwnerSelector.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import {
		DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH,
		DAILY_TEACHING_TEACHER_COLUMN_WIDTH,
		dailyTeachingCardPresentation,
		dailyTeachingEntryTitle,
		dailyTeachingEmptyCellLabel,
		dailyTeachingRows,
		filterDailyTeachingRows,
		dailyTeachingGroupInstructors,
		type DailyTeachingDisplayRow,
		type DailyTeachingDisplayCell,
		type DailyTeachingView,
		dailyTeachingTableMinWidth,
		groupDailyTeachingEntries
	} from '#lib/utils/daily-teaching-display.js';
	import {
		CalendarClock,
		ChevronLeft,
		ChevronRight,
		ExternalLink,
		DoorOpen,
		Users,
		MapPin,
		Printer,
		Search,
		School,
		UsersRound
	} from '@lucide/svelte';
	import type { PageProps } from './$types';

	type SelectedCell = {
		row: DailyTeachingDisplayRow;
		cell: DailyTeachingDisplayCell;
		period: DailyTeachingPeriod;
		entries: DailyTeachingEntry[];
	};

	let { data }: PageProps = $props();
	const academicTermId = $derived(data.academicTermId);
	let overview = $state<DailyTeachingOverview | null>(null);
	let selectedDate = $state('');
	let includeEmptyTeachers = $state(false);
	let entrySearch = $state('');
	const presentationUrl = $derived(page.shallow?.url ?? page.url);
	const view = $derived<DailyTeachingView>(
		presentationUrl.searchParams.get('view') === 'homeroom' ? 'homeroom' : 'teacher'
	);
	const ownerId = $derived(presentationUrl.searchParams.get('owner')?.trim() || 'all');
	const ownerLabel = $derived(view === 'teacher' ? 'ครู' : 'ห้องเรียน');
	let loading = $state(true);
	let errorMessage = $state('');
	let selectedCell = $state<SelectedCell | null>(null);
	let cellDialogOpen = $state(false);
	const request = new LatestRequest();
	let activeTermId: string | null = null;
	let activeOverviewKey = '';
	let interactionRevision = 0;
	onDestroy(() => request.abort());

	const canReadDailyTeaching = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_PUBLISH_SCHOOL
		)
	);
	const canOpenPlanner = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_TIMETABLE_PUBLISH_SCHOOL
		)
	);

	const rows = $derived(dailyTeachingRows(overview?.teachers ?? [], view));
	const ownerOptions = $derived.by(() => {
		const options = rows.map((row) => ({
			id: row.id,
			label: row.displayName,
			code: '',
			kind: row.kind
		}));
		if (ownerId !== 'all' && !rows.some((row) => row.id === ownerId)) {
			options.unshift({
				id: ownerId,
				label: `ไม่พบ${ownerLabel}ที่เลือกในวันนี้`,
				code: '',
				kind: view
			});
		}
		return [{ id: 'all', label: `${ownerLabel}ทั้งหมด`, code: '', kind: view }, ...options];
	});
	const filteredRows = $derived(filterDailyTeachingRows(rows, ownerId, entrySearch));

	function choosePresentation(
		nextView: DailyTeachingView,
		nextOwner = nextView === view ? ownerId : 'all'
	): void {
		const query = new SvelteURLSearchParams(presentationUrl.search);
		if (nextView === 'teacher') query.delete('view');
		else query.set('view', nextView);
		if (nextOwner === 'all') query.delete('owner');
		else query.set('owner', nextOwner);
		void goto(`${resolve('/(app)/staff/academic/timetable/today')}?${query}`, {
			shallow: true,
			state: page.state
		});
	}
	function chooseView(nextView: string): void {
		if (nextView !== 'teacher' && nextView !== 'homeroom') return;
		choosePresentation(nextView);
	}

	const tableMinWidth = $derived(dailyTeachingTableMinWidth(overview?.periods.length ?? 4));

	async function loadOverview(termId = academicTermId): Promise<void> {
		if (!termId || !canReadDailyTeaching) return;
		const key = `${termId}:${selectedDate}:${includeEmptyTeachers}`;
		const { revision, signal } = request.begin();
		interactionRevision += 1;
		if (activeOverviewKey !== key) {
			activeOverviewKey = key;
			overview = null;
		}
		loading = true;
		errorMessage = '';
		try {
			const loaded = await getDailyTeachingOverview(
				{
					academicTermId: termId,
					date: selectedDate,
					includeEmptyTeachers
				},
				{ signal }
			);
			if (request.isCurrent(revision) && academicTermId === termId) overview = loaded;
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดตารางสอนรายวันไม่สำเร็จ';
				toast.error(errorMessage);
			}
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	function handleDateChange(value: string | undefined): void {
		if (!value) return;
		selectedDate = value;
		void loadOverview();
	}

	function moveDate(offset: number): void {
		const next = parseDateInput(selectedDate);
		next.setDate(next.getDate() + offset);
		selectedDate = toDateInputValue(next);
		void loadOverview();
	}

	function toggleEmptyTeachers(): void {
		includeEmptyTeachers = !includeEmptyTeachers;
		void loadOverview();
	}

	function cellForPeriod(row: DailyTeachingDisplayRow, periodId: string): DailyTeachingDisplayCell {
		return (
			row.periods.find((cell) => cell.bellSchedulePeriodId === periodId) ?? {
				bellSchedulePeriodId: periodId,
				entries: [],
				instructorsByEntry: new Map()
			}
		);
	}

	function entryTitle(entry: DailyTeachingEntry): string {
		return entry.title?.trim() || dailyTeachingEntryTitle(entry);
	}

	function entryDetailLabels(entry: DailyTeachingEntry): string[] {
		const title = entryTitle(entry);
		return [entry.offeringName, entry.subjectVersionDisplayLabel, entry.activityVersionDisplayLabel]
			.filter((value): value is string => Boolean(value?.trim()) && value !== title)
			.filter((value, index, values) => values.indexOf(value) === index);
	}

	function entrySubtitle(entry: DailyTeachingEntry): string {
		return [entry.learningGroupName, entry.homeroomNames.join(', ')].filter(Boolean).join(' · ');
	}

	function periodLabel(period: DailyTeachingPeriod): string {
		return period.name ?? `คาบ ${period.orderIndex}`;
	}

	function periodTime(period: DailyTeachingPeriod): string {
		return `${period.startTime.slice(0, 5)}–${period.endTime.slice(0, 5)}`;
	}

	function cellAriaLabel(
		row: DailyTeachingDisplayRow,
		period: DailyTeachingPeriod,
		entries: DailyTeachingEntry[]
	): string {
		const label = periodLabel(period);
		const time = periodTime(period);
		if (entries.length === 0) {
			return dailyTeachingEmptyCellLabel(row.displayName, label, time);
		}
		const titles = Array.from(
			new Set(
				groupDailyTeachingEntries(entries, row.kind).map((group) => {
					const presentation = dailyTeachingCardPresentation(
						group,
						cellForPeriod(row, period.id),
						row.kind
					);
					return [presentation.code, presentation.title].filter(Boolean).join(' ');
				})
			)
		);
		return `${row.displayName} ${label} ${time}: ${titles.join(', ')}`;
	}

	function openCell(
		row: DailyTeachingDisplayRow,
		period: DailyTeachingPeriod,
		cell: DailyTeachingDisplayCell
	): void {
		selectedCell = { row, period, cell, entries: cell.entries };
		cellDialogOpen = true;
	}

	function formatDate(value: string): string {
		return parseDateInput(value).toLocaleDateString('th-TH', {
			weekday: 'long',
			day: 'numeric',
			month: 'long',
			year: 'numeric'
		});
	}

	function parseDateInput(value: string): Date {
		const [year, month, day] = value.split('-').map(Number);
		return new Date(year, month - 1, day);
	}

	function toDateInputValue(date: Date): string {
		const year = date.getFullYear();
		const month = String(date.getMonth() + 1).padStart(2, '0');
		const day = String(date.getDate()).padStart(2, '0');
		return `${year}-${month}-${day}`;
	}

	$effect.pre(() => {
		const termId = data.academicTermId;
		const initialDate = data.initialDate;
		const routeOverview = data.overview;
		const routeKey = `${termId}:${initialDate}:false`;
		const initialInteractionRevision = interactionRevision;
		let current = true;
		untrack(() => {
			if (activeTermId !== termId) {
				activeTermId = termId;
				selectedDate = initialDate;
				includeEmptyTeachers = false;
				overview = null;
				activeOverviewKey = routeKey;
				selectedCell = null;
				cellDialogOpen = false;
			}
			if (activeOverviewKey === routeKey) {
				request.abort();
				loading = Boolean(routeOverview);
				errorMessage = '';
			}
		});
		if (routeOverview && activeOverviewKey === routeKey) {
			void routeOverview.then((result) => {
				if (!current) return;
				untrack(() => {
					if (interactionRevision === initialInteractionRevision) {
						if (result.ok) overview = result.data;
						else errorMessage = result.error;
						loading = false;
					}
				});
			});
		}
		return () => {
			current = false;
		};
	});
</script>

<PageShell title="ตารางสอนวันนี้" description="ตารางรายวัน แยกตามครูหรือห้องเรียน">
	<header
		class="print:hidden overflow-hidden rounded-xl border bg-background shadow-sm"
		aria-label="เครื่องมือตารางสอนรายวัน"
	>
		<div class="h-1 bg-primary"></div>
		<div class="flex flex-wrap items-end gap-3 p-3">
			<div class="space-y-1.5">
				<Label for="teaching-date" class="text-xs">วันที่</Label>
				<div class="flex gap-1.5">
					<Button
						variant="outline"
						size="icon"
						aria-label="วันก่อนหน้า"
						onclick={() => moveDate(-1)}><ChevronLeft /></Button
					>
					<DatePicker
						id="teaching-date"
						value={selectedDate}
						onValueChange={handleDateChange}
						class="min-w-48"
					/>
					<Button variant="outline" size="icon" aria-label="วันถัดไป" onclick={() => moveDate(1)}
						><ChevronRight /></Button
					>
				</div>
			</div>
			<ToggleGroup.Root
				type="single"
				class="rounded-lg border bg-muted/45 p-1"
				aria-label="มุมมองตารางสอนรายวัน"
				bind:value={() => view, chooseView}
			>
				<ToggleGroup.Item
					value="teacher"
					class="h-8 rounded-md border-transparent px-3"
					aria-label="ครู"><UsersRound class="size-3.5" aria-hidden="true" />ครู</ToggleGroup.Item
				>
				<ToggleGroup.Item
					value="homeroom"
					class="h-8 rounded-md border-transparent px-3"
					aria-label="ห้องเรียน"
					><School class="size-3.5" aria-hidden="true" />ห้องเรียน</ToggleGroup.Item
				>
			</ToggleGroup.Root>
			<div class="w-full min-w-0 space-y-1.5 sm:w-56">
				<p class="text-xs font-medium">เลือก{ownerLabel}</p>
				<TimetableOwnerSelector
					value={ownerId}
					options={ownerOptions}
					onValueChange={(value) => choosePresentation(view, value)}
					ariaLabel={`เลือก${ownerLabel}`}
					searchPlaceholder={`ค้นหา${ownerLabel}...`}
					disabled={!overview}
				/>
			</div>
			<div
				class="w-full min-w-0 flex-none space-y-1.5 sm:min-w-44 sm:flex-1"
				data-testid="daily-search-field"
			>
				<Label for="entry-search" class="text-xs">ค้นหาวิชา กิจกรรม หรือกลุ่มเรียน</Label>
				<div class="relative">
					<Search class="text-muted-foreground absolute top-2.5 left-3 size-4" />
					<Input id="entry-search" bind:value={entrySearch} class="pl-9 text-xs" />
				</div>
			</div>
			<div class="flex flex-wrap items-center gap-2" data-testid="daily-toolbar-actions">
				{#if view === 'teacher'}
					<div class="flex h-9 items-center gap-2 rounded-md border px-2">
						<Switch
							id="include-empty"
							checked={includeEmptyTeachers}
							onclick={toggleEmptyTeachers}
						/>
						<Label for="include-empty" class="text-xs">รวมครูที่ไม่มีคาบ</Label>
					</div>
				{/if}
				{#if canOpenPlanner}<Button
						variant="outline"
						size="sm"
						href={academicContextualMenuPath(
							'/staff/academic/timetable',
							{ academicYearId: data.academicYearId, academicTermId },
							null
						)}
						data-sveltekit-preload-data="tap"><ExternalLink />จัดตาราง</Button
					>{/if}
				<Button variant="outline" size="sm" disabled={!overview} onclick={() => window.print()}
					><Printer />พิมพ์</Button
				>
			</div>
		</div>
	</header>

	{#if !canReadDailyTeaching}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูตารางสอนวันนี้"
			description="บัญชีนี้ยังไม่ได้รับสิทธิ์ดูภาพรวมตารางสอนรายวันของโรงเรียน"
		/>
	{:else if !academicTermId}
		<PageState
			variant="empty"
			title="เลือกภาคเรียนก่อน"
			description="ใช้ตัวเลือกปีการศึกษาและภาคเรียนบนแถบด้านบน"
		/>
	{:else if loading && !overview}
		<PageSkeleton variant="table" rows={8} columns={6} />
	{:else if errorMessage && !overview}
		<PageState
			variant="error"
			title="โหลดตารางสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองใหม่"
			onaction={() => loadOverview()}
		/>
	{:else if !overview}
		<PageState
			title="ยังไม่มีข้อมูลตารางสอน"
			description="ภาคเรียนหรือวันที่นี้ยังไม่มีคาบที่แสดงได้"
		/>
	{:else}
		<div class="relative space-y-4" aria-busy={loading} data-testid="daily-teaching-ready">
			{#if errorMessage}
				<div
					role="alert"
					class="flex flex-wrap items-center gap-3 rounded-lg border border-destructive/40 p-3 text-sm"
				>
					<span>{errorMessage}</span>
					<Button variant="outline" size="sm" onclick={() => loadOverview()}>ลองใหม่</Button>
				</div>
			{/if}

			<section class="overflow-hidden rounded-md border bg-background">
				<div class="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 border-b px-3 py-2">
					<h2 class="flex items-center gap-2 text-sm font-semibold">
						<CalendarClock class="size-4" />{formatDate(overview.date)}
					</h2>
					<p class="text-muted-foreground text-xs">
						แสดง {filteredRows.length}/{rows.length}
						{ownerLabel} · {overview.periods.length} คาบ · ครูที่มีคาบ {overview.summary
							.teachersTeachingCount} คน · {overview.summary.lessonCount} คาบสอน
					</p>
				</div>
				{#if filteredRows.length === 0}
					<PageState
						title={`ไม่พบ${ownerLabel}ตามตัวเลือก`}
						description={view === 'teacher'
							? 'ลองเลือกครูทั้งหมด เปลี่ยนคำค้น หรือรวมครูที่ไม่มีคาบ'
							: 'ลองเลือกห้องเรียนทั้งหมดหรือเปลี่ยนคำค้น เฉพาะคาบที่ระบุห้องประจำชั้นจะแสดงในมุมมองนี้'}
						class="m-4"
					/>
				{:else}
					<div class="daily-teaching-scroll max-h-[70vh] overflow-auto">
						<Table.Root
							class="daily-teaching-table border-0"
							style={`--teacher-column-width: ${DAILY_TEACHING_TEACHER_COLUMN_WIDTH}px; --minimum-period-column-width: ${DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH}px; min-width: ${tableMinWidth}px;`}
						>
							<colgroup>
								<col class="daily-teaching-teacher-column" />
								{#each overview.periods as period (period.id)}<col />{/each}
							</colgroup>
							<Table.Header class="sticky top-0 z-40"
								><Table.Row class="bg-muted/80 hover:bg-muted/80"
									><Table.Head
										class="daily-teaching-teacher-column sticky left-0 z-50 h-9 overflow-hidden bg-muted px-1.5 text-[10px]"
										>{ownerLabel}</Table.Head
									>{#each overview.periods as period (period.id)}<Table.Head
											class="daily-teaching-period-column h-9 overflow-hidden bg-muted px-1 text-center text-[10px]"
											><p class="truncate">{period.name ?? `คาบ ${period.orderIndex}`}</p>
											<p class="text-muted-foreground truncate text-[9px] font-normal">
												{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
											</p></Table.Head
										>{/each}</Table.Row
								></Table.Header
							>
							<Table.Body>
								{#each filteredRows as row (row.id)}
									<Table.Row>
										<Table.Cell
											class="daily-teaching-teacher-column sticky left-0 z-20 overflow-hidden bg-background px-1.5 py-1 text-[10px] font-medium"
											><p class="truncate" title={row.displayName}>
												{row.displayName}
											</p></Table.Cell
										>
										{#each overview.periods as period (period.id)}
											{@const cell = cellForPeriod(row, period.id)}
											<Table.Cell
												class="daily-teaching-period-column h-px overflow-hidden p-0.5 align-top"
											>
												<button
													type="button"
													class="hover:bg-muted/30 grid h-full min-h-[88px] w-full min-w-0 auto-rows-fr gap-0.5 overflow-hidden rounded-md p-0.5 text-left"
													aria-label={cellAriaLabel(row, period, cell.entries)}
													onclick={() => openCell(row, period, cell)}
												>
													{#each groupDailyTeachingEntries(cell.entries, view) as group (group.key)}
														{@const presentation = dailyTeachingCardPresentation(group, cell, view)}
														<article
															class="flex min-h-full min-w-0 flex-col overflow-hidden rounded-lg border bg-background p-1.5 text-left shadow-xs"
															data-testid="daily-lesson-card"
														>
															{#if presentation.code}<p
																	data-daily-card-code
																	class="truncate text-[10px] leading-[14px] font-semibold"
																	title={presentation.code}
																>
																	{presentation.code}
																</p>{/if}
															<h3
																data-daily-card-title
																class={presentation.titleLineLimit === 3
																	? 'line-clamp-3 whitespace-pre-line text-[11px] leading-[16px] font-semibold'
																	: 'truncate text-[11px] leading-[16px] font-semibold'}
																title={presentation.title}
															>
																{presentation.title}
															</h3>
															<div
																class="mt-1 min-w-0 space-y-0.5 text-[9px] leading-[14px] text-muted-foreground"
															>
																{#if presentation.targetLabel}<p
																		data-daily-card-target
																		class="truncate"
																		title={presentation.targetLabel}
																	>
																		{presentation.targetLabel}
																	</p>{/if}
																{#if presentation.teacherLabel}<p
																		data-daily-card-teachers
																		class="flex min-w-0 items-center gap-1"
																		title={presentation.teacherTitle ?? undefined}
																	>
																		<Users class="size-3 shrink-0" aria-hidden="true" /><span
																			class="min-w-0 truncate">{presentation.teacherLabel}</span
																		>
																	</p>{/if}
																{#if presentation.roomLabel}<p
																		data-daily-card-room
																		class="flex min-w-0 items-center gap-1"
																		title={presentation.roomLabel}
																	>
																		<DoorOpen class="size-3 shrink-0" aria-hidden="true" /><span
																			class="min-w-0 truncate">{presentation.roomLabel}</span
																		>
																	</p>{/if}
															</div>
														</article>
													{/each}
												</button>
											</Table.Cell>
										{/each}
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
					</div>
				{/if}
			</section>
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={cellDialogOpen}>
	<Dialog.Content class="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header
			><Dialog.Title>{selectedCell?.row.displayName ?? 'รายละเอียดคาบ'}</Dialog.Title
			><Dialog.Description
				>{selectedCell
					? `${periodLabel(selectedCell.period)} · ${periodTime(selectedCell.period)}`
					: ''}</Dialog.Description
			></Dialog.Header
		>
		<div class="space-y-3 py-2">
			{#if selectedCell?.entries.length === 0}<p class="text-muted-foreground text-sm">
					ไม่มีคาบสอนในช่วงเวลานี้
				</p>{/if}
			{#each selectedCell?.entries ?? [] as entry (entry.entryId)}
				{@const detailTitle = entryTitle(entry)}
				<div class="rounded-lg border p-4" data-daily-entry-id={entry.entryId}>
					<div class="flex items-center justify-between gap-3">
						<div class="min-w-0">
							<p class="font-medium">{detailTitle}</p>
							{#each entryDetailLabels(entry) as label (label)}
								<p class="text-muted-foreground mt-1 text-sm">{label}</p>
							{/each}
						</div>
						<div class="flex shrink-0 flex-wrap justify-end gap-1">
							{#if entry.offeringCode && entry.offeringCode !== detailTitle}
								<Badge variant="outline">{entry.offeringCode}</Badge>
							{/if}
							<Badge variant="outline">{entry.entryType}</Badge>
						</div>
					</div>
					<p class="mt-2 text-sm">
						ครูผู้สอน: {selectedCell
							? dailyTeachingGroupInstructors([entry], selectedCell.cell)
									.map((instructor) => instructor.displayName)
									.join(', ')
							: ''}
					</p>
					<p class="text-muted-foreground mt-2 text-sm">
						{entrySubtitle(entry) || 'ไม่มีข้อมูลกลุ่มเรียน'}
					</p>
					{#if entry.note}<p class="mt-2 text-sm">
							{entry.note}
						</p>{/if}{#if entry.roomName?.trim() || entry.roomId}<p
							class="mt-2 flex items-center gap-1 text-sm"
						>
							<MapPin class="size-4" />
							{entry.roomName?.trim() || 'ไม่พบชื่อห้อง'}
						</p>{/if}
				</div>
			{/each}
		</div>
	</Dialog.Content>
</Dialog.Root>

<style>
	:global(.daily-teaching-table) {
		table-layout: fixed;
		width: 100%;
	}

	:global(.daily-teaching-teacher-column) {
		width: var(--teacher-column-width);
		min-width: var(--teacher-column-width);
		max-width: var(--teacher-column-width);
	}

	:global(.daily-teaching-table th),
	:global(.daily-teaching-table td) {
		max-width: 0;
	}

	:global(.daily-teaching-table .daily-teaching-teacher-column) {
		max-width: var(--teacher-column-width);
	}

	:global(.daily-teaching-scroll [data-slot='table-container']) {
		overflow: visible;
	}
</style>
