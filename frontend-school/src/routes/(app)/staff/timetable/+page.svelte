<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import {
		currentLocalDate,
		getMyTimetable,
		periodsFromTimetableBlocks,
		type TimetableBlock,
		type TimetablePeriodSummary
	} from '$lib/api/timetable';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { authStore } from '$lib/stores/auth';
	import { generateTimetablePDF } from '$lib/utils/pdf';
	import {
		buildStaffOwnTimetablePdfDownload,
		canDownloadStaffOwnTimetablePdf,
		runStaffOwnTimetablePdfDownload,
		staffOwnTimetableSelectionKey
	} from '$lib/utils/staff-own-timetable-pdf';
	import { Download, Loader2, MapPin, School } from 'lucide-svelte';

	const dayOptions = [
		{ value: 'MON', label: 'จันทร์' },
		{ value: 'TUE', label: 'อังคาร' },
		{ value: 'WED', label: 'พุธ' },
		{ value: 'THU', label: 'พฤหัสบดี' },
		{ value: 'FRI', label: 'ศุกร์' },
		{ value: 'SAT', label: 'เสาร์' },
		{ value: 'SUN', label: 'อาทิตย์' }
	];

	const academicContext = getAcademicContextStore();
	const academicYearId = $derived($academicContext.selected.academicYearId ?? '');
	const academicTermId = $derived($academicContext.selected.academicTermId);
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
	let loading = $state(false);
	let isExportingPdf = $state(false);
	let loadedSelectionKey = $state('');
	let errorMessage = $state('');
	let revision = 0;

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

	async function loadTimetable(termId: string): Promise<void> {
		const current = ++revision;
		loading = true;
		loadedSelectionKey = '';
		errorMessage = '';
		try {
			const loaded = await getMyTimetable({
				academicTermId: termId,
				date: currentLocalDate()
			});
			if (current === revision) {
				periods = periodsFromTimetableBlocks(loaded);
				blocks = loaded;
				loadedSelectionKey = staffOwnTimetableSelectionKey(academicYearId, termId);
			}
		} catch (error) {
			if (current === revision) {
				errorMessage = error instanceof Error ? error.message : 'โหลดตารางสอนไม่สำเร็จ';
			}
		} finally {
			if (current === revision) loading = false;
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

	function blocksForCell(day: string, periodId: string): TimetableBlock[] {
		return blocks.filter(
			(block) => block.dayOfWeek === day && block.bellSchedulePeriodId === periodId
		);
	}

	function blockTitle(block: TimetableBlock): string {
		return block.offeringCode ?? block.title ?? 'กิจกรรม';
	}

	function blockColor(blockKind: TimetableBlock['blockKind']): string {
		if (blockKind === 'course')
			return 'border-blue-200 bg-blue-50 text-blue-950 dark:border-blue-800 dark:bg-blue-950/40 dark:text-blue-100';
		if (blockKind === 'activity')
			return 'border-emerald-200 bg-emerald-50 text-emerald-950 dark:border-emerald-800 dark:bg-emerald-950/40 dark:text-emerald-100';
		return 'border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-800 dark:bg-amber-950/40 dark:text-amber-100';
	}

	function groupLabel(block: TimetableBlock): string {
		return [
			...block.groups.map((group) => group.name),
			...block.homerooms.map((room) => room.name)
		].join(', ');
	}

	function roomLabel(block: TimetableBlock): string {
		return [
			...block.groups.map((group) => group.roomCode),
			...block.homerooms.map((room) => room.roomCode)
		]
			.filter(Boolean)
			.join(', ');
	}

	onMount(() => {
		let loadedTermId: string | null = null;
		return academicContext.subscribe((state) => {
			const termId = state.selected.academicTermId;
			if (termId && termId !== loadedTermId) {
				loadedTermId = termId;
				void loadTimetable(termId);
			}
		});
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
	{:else if loading}
		<PageSkeleton variant="table" rows={6} columns={Math.max(periods.length + 1, 4)} />
	{:else if errorMessage}
		<PageState
			variant="error"
			title="โหลดตารางสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadTimetable(academicTermId)}
		/>
	{:else if blocks.length === 0}
		<PageState title="ยังไม่มีตารางสอน" description="ยังไม่มีคาบสอนของคุณในภาคเรียนนี้" />
	{:else}
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
										<div
											class={`mb-1 flex min-h-20 flex-col rounded-md border p-2 text-xs ${blockColor(block.blockKind)}`}
										>
											<p class="truncate font-semibold">{blockTitle(block)}</p>
											{#if block.offeringName}<p class="mt-1 line-clamp-2 opacity-80">
													{block.offeringName}
												</p>{/if}
											{#if groupLabel(block)}<p
													class="mt-auto flex items-center gap-1 truncate opacity-70"
												>
													<School class="size-3" />
													{groupLabel(block)}
												</p>{/if}
											{#if roomLabel(block)}<p class="flex items-center gap-1 truncate opacity-70">
													<MapPin class="size-3" />
													{roomLabel(block)}
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
	{/if}
</PageShell>
