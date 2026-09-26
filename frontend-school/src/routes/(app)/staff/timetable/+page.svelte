<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import { buildTimetableBlockDisplay } from '$lib/academic/timetable/block-display';
	import {
		currentLocalDate,
		getMyTimetable,
		periodsFromTimetableBlocks,
		type TimetableBlock,
		type TimetablePeriodSummary
	} from '$lib/api/timetable';
	import { PageShell } from '$lib/components/app-layout';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { PageSkeleton, PageState, RegionUpdatingState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { authStore } from '$lib/stores/auth';
	import {
		buildStaffOwnTimetablePdfDownload,
		canDownloadStaffOwnTimetablePdf,
		runStaffOwnTimetablePdfDownload,
		staffOwnTimetableSelectionKey
	} from '$lib/utils/staff-own-timetable-pdf';
	import { Download, Loader2, MapPin, School } from '@lucide/svelte';
	import type { PageProps } from './$types';

	const dayOptions = [
		{ value: 'MON', label: 'จันทร์' },
		{ value: 'TUE', label: 'อังคาร' },
		{ value: 'WED', label: 'พุธ' },
		{ value: 'THU', label: 'พฤหัสบดี' },
		{ value: 'FRI', label: 'ศุกร์' },
		{ value: 'SAT', label: 'เสาร์' },
		{ value: 'SUN', label: 'อาทิตย์' }
	];

	let { data }: PageProps = $props();
	const academicContext = getAcademicContextStore();
	const academicYearId = $derived(data.academicYearId ?? '');
	const academicTermId = $derived(data.academicTermId);
	const selectedYear = $derived(
		$academicContext.options?.years.find((year) => year.id === academicYearId) ?? null
	);
	const selectedTerm = $derived(
		$academicContext.options?.terms.find((term) => term.id === academicTermId) ?? null
	);
	const userName = $derived(
		$authStore.user
			? `${$authStore.user.firstName ?? ''} ${$authStore.user.lastName ?? ''}`.trim()
			: ''
	);
	let blocks = $state<TimetableBlock[]>([]);
	let periods = $state<TimetablePeriodSummary[]>([]);
	let loading = $state(true);
	let isExportingPdf = $state(false);
	let loadedSelectionKey = $state('');
	let errorMessage = $state('');
	const request = new LatestRequest();
	let activeSelectionKey = '';
	let interactionRevision = 0;
	onDestroy(() => request.abort());

	const schoolDays = $derived.by(() => {
		const configured = new Set(blocks.map((block) => block.dayOfWeek));
		return configured.size > 0
			? dayOptions.filter((day) => configured.has(day.value))
			: dayOptions.slice(0, 5);
	});
	const tableMinWidth = $derived(96 + periods.length * 132);
	const canDownloadPdf = $derived(
		canDownloadStaffOwnTimetablePdf({
			loading,
			isExporting: isExportingPdf,
			selectedYearId: academicYearId,
			selectedAcademicTermId: academicTermId ?? '',
			selectedTermYearId: selectedTerm?.academicYearId,
			loadedSelectionKey,
			blockCount: blocks.length,
			periodCount: periods.length
		})
	);

	function applyBlocks(loaded: TimetableBlock[], yearId: string, termId: string): void {
		periods = periodsFromTimetableBlocks(loaded);
		blocks = loaded;
		loadedSelectionKey = staffOwnTimetableSelectionKey(yearId, termId);
	}

	async function loadTimetable(termId = academicTermId): Promise<void> {
		if (!termId) return;
		const yearId = academicYearId;
		const { revision, signal } = request.begin();
		interactionRevision += 1;
		loading = true;
		errorMessage = '';
		try {
			const loaded = await getMyTimetable(
				{
					academicTermId: termId,
					date: currentLocalDate()
				},
				{ signal }
			);
			if (request.isCurrent(revision) && academicYearId === yearId && academicTermId === termId)
				applyBlocks(loaded, yearId, termId);
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดตารางสอนไม่สำเร็จ';
			}
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function downloadPdf(): Promise<void> {
		if (!canDownloadPdf || !selectedTerm || !selectedYear) return;
		const download = buildStaffOwnTimetablePdfDownload({
			teacherName: userName,
			termName: selectedTerm.name,
			termCode: selectedTerm.code,
			academicYearName: selectedYear.name,
			blocks,
			dayValues: schoolDays.map((day) => day.value),
			periods
		});

		isExportingPdf = true;
		try {
			const { generateTimetablePDF } = await import('$lib/utils/pdf');
			await runStaffOwnTimetablePdfDownload(download, {
				generatePdf: generateTimetablePDF,
				setExporting: (value) => (isExportingPdf = value),
				onSuccess: () => toast.success('ดาวน์โหลดตารางสอนแล้ว'),
				onError: (error) => {
					console.error('Failed to download timetable PDF', error);
					toast.error('ดาวน์โหลดตารางสอนไม่สำเร็จ');
				}
			});
		} catch (error) {
			console.error('Failed to load timetable PDF module', error);
			toast.error('ดาวน์โหลดตารางสอนไม่สำเร็จ');
		} finally {
			isExportingPdf = false;
		}
	}

	function blocksForCell(day: string, periodId: string): TimetableBlock[] {
		return blocks.filter(
			(block) => block.dayOfWeek === day && block.bellSchedulePeriodId === periodId
		);
	}

	function blockCode(block: TimetableBlock): string | null {
		return block.blockKind === 'course' ? block.offeringCode : null;
	}

	function blockTitle(block: TimetableBlock): string {
		if (block.blockKind === 'structural') return block.title ?? 'กิจกรรม';
		return block.offeringName ?? block.title ?? 'กิจกรรม';
	}

	function blockColor(blockKind: TimetableBlock['blockKind']): string {
		if (blockKind === 'course')
			return 'border-blue-200 bg-blue-50 text-blue-950 dark:border-blue-800 dark:bg-blue-950/40 dark:text-blue-100';
		if (blockKind === 'activity')
			return 'border-emerald-200 bg-emerald-50 text-emerald-950 dark:border-emerald-800 dark:bg-emerald-950/40 dark:text-emerald-100';
		return 'border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-800 dark:bg-amber-950/40 dark:text-amber-100';
	}

	$effect.pre(() => {
		const yearId = data.academicYearId ?? '';
		const termId = data.academicTermId;
		const routeBlocks = data.blocks;
		const selectionKey = staffOwnTimetableSelectionKey(yearId, termId ?? '');
		const initialInteractionRevision = interactionRevision;
		let current = true;
		untrack(() => {
			if (activeSelectionKey !== selectionKey) {
				activeSelectionKey = selectionKey;
				blocks = [];
				periods = [];
				loadedSelectionKey = '';
			}
			request.abort();
			loading = Boolean(routeBlocks);
			errorMessage = '';
		});
		if (routeBlocks && termId) {
			void routeBlocks.then((result) => {
				if (!current) return;
				untrack(() => {
					if (interactionRevision === initialInteractionRevision) {
						if (result.ok) applyBlocks(result.data, yearId, termId);
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

<PageShell title="ตารางสอน" description={userName ? `ครู${userName}` : 'ตารางสอนของฉัน'}>
	{#snippet actions()}
		<Button variant="outline" disabled={!canDownloadPdf} onclick={downloadPdf}>
			{#if isExportingPdf}<Loader2 class="animate-spin" />{:else}<Download />{/if}
			ดาวน์โหลด PDF
		</Button>
	{/snippet}

	{#if !academicTermId}
		<PageState
			variant="empty"
			title="เลือกภาคเรียนก่อน"
			description="ใช้ตัวเลือกปีการศึกษาและภาคเรียนบนแถบด้านบน"
		/>
	{:else if loading && blocks.length === 0}
		<PageSkeleton variant="table" rows={6} columns={Math.max(periods.length + 1, 4)} />
	{:else if errorMessage && blocks.length === 0}
		<PageState
			variant="error"
			title="โหลดตารางสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadTimetable()}
		/>
	{:else if blocks.length === 0}
		<PageState title="ยังไม่มีตารางสอน" description="ยังไม่มีคาบสอนของคุณในภาคเรียนนี้" />
	{:else}
		<div class="relative" aria-busy={loading} data-testid="personal-timetable-ready">
			{#if loading}<RegionUpdatingState />{/if}
			{#if errorMessage}
				<div
					role="alert"
					class="mb-3 flex flex-wrap items-center gap-3 rounded-lg border border-destructive/40 p-3 text-sm"
				>
					<span>{errorMessage}</span>
					<Button variant="outline" size="sm" onclick={() => loadTimetable()}>ลองอีกครั้ง</Button>
				</div>
			{/if}
			<div class="overflow-x-auto rounded-lg border">
				<table class="w-full table-fixed border-collapse" style={`min-width: ${tableMinWidth}px`}>
					<thead
						><tr
							><th class="bg-muted/70 w-24 border p-2 text-xs">วัน / คาบ</th
							>{#each periods as period, index (period.id)}<th
									class="bg-muted/70 border p-2 text-center text-xs"
									><p class="font-semibold">{period.name ?? `คาบ ${index + 1}`}</p>
									<p class="text-muted-foreground font-normal">
										{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
									</p></th
								>{/each}</tr
						></thead
					>
					<tbody>
						{#each schoolDays as day (day.value)}
							<tr
								><th class="bg-muted/30 border p-2 text-xs">{day.label}</th>
								{#each periods as period (period.id)}
									{@const cellBlocks = blocksForCell(day.value, period.id)}
									<td class="h-24 border p-1 align-top">
										{#each cellBlocks as block (block.id)}
											{@const display = buildTimetableBlockDisplay(block, 'personal')}
											{@const code = blockCode(block)}
											<div
												class={`flex h-full min-h-20 flex-col rounded-md border p-2 text-xs ${blockColor(block.blockKind)}`}
											>
												{#if code}<p class="truncate font-semibold">{code}</p>{/if}
												<p
													class={[
														code && 'mt-1',
														block.blockKind === 'course'
															? 'line-clamp-2 opacity-80'
															: 'line-clamp-3 whitespace-pre-line font-semibold'
													]}
												>
													{blockTitle(block)}
												</p>
												{#if display.groupLabel}<p
														class="mt-auto flex items-center gap-1 truncate opacity-70"
													>
														<School class="size-3" />
														{display.groupLabel}
													</p>{/if}
												{#if display.roomLabel}<p
														class="flex items-center gap-1 truncate opacity-70"
													>
														<MapPin class="size-3" />
														{display.roomLabel}
													</p>{/if}
											</div>
										{/each}
									</td>
								{/each}
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</PageShell>
