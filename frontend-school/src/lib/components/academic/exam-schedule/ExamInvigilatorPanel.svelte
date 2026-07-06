<script lang="ts">
	import type {
		ExamDayDetail,
		ExamInvigilatorAssignmentSummary,
		ExamInvigilatorStaffOption,
		ExamInvigilatorStaffWorkload,
		ExamInvigilatorWorkspace,
		InvigilatorView
	} from '$lib/api/examSchedule';
	import { LoadingButton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Sheet from '$lib/components/ui/sheet';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import { Search, UserRoundPlus } from 'lucide-svelte';

	type StaffOption = {
		id: string;
		displayName: string;
		detail?: string;
	};

	let {
		days = [],
		workspace,
		staff = [],
		loading = false,
		loadError = '',
		readonly = false,
		savingAssignmentId = null,
		onSaveInvigilators,
		onSearchStaff,
		onRetry
	}: {
		days: ExamDayDetail[];
		workspace: ExamInvigilatorWorkspace | null;
		staff: ExamInvigilatorStaffOption[];
		loading?: boolean;
		loadError?: string;
		readonly?: boolean;
		savingAssignmentId?: string | null;
		onSaveInvigilators?: (assignmentId: string, staffIds: string[]) => Promise<boolean> | boolean;
		onSearchStaff?: (search: string) => Promise<ExamInvigilatorStaffOption[]>;
		onRetry?: () => Promise<void> | void;
	} = $props();

	let selectedDayId = $state('');
	let editorOpen = $state(false);
	let selectedAssignmentId = $state<string | null>(null);
	let selectedStaffIds = $state<string[]>([]);
	let staffSearch = $state('');
	let staffOptions = $state<ExamInvigilatorStaffOption[]>([]);
	let selectedStaffCache = $state<ExamInvigilatorStaffOption[]>([]);
	let staffSearchLoading = $state(false);
	let staffSearchError = $state('');
	let staffSearchRequestToken = 0;

	const sortedDays = $derived([...days].sort((a, b) => a.sortOrder - b.sortOrder));
	const selectedDay = $derived(
		days.find((day) => day.id === selectedDayId) ?? sortedDays[0] ?? null
	);
	const selectedDayAssignments = $derived(
		[...(workspace?.assignments ?? [])]
			.filter((assignment) => assignment.examDayId === (selectedDay?.id ?? selectedDayId))
			.sort((a, b) => {
				const classroomCompare = a.classroomName.localeCompare(b.classroomName, 'th');
				return classroomCompare === 0
					? a.roomName.localeCompare(b.roomName, 'th')
					: classroomCompare;
			})
	);
	const selectedAssignment = $derived(
		workspace?.assignments.find((assignment) => assignment.assignmentId === selectedAssignmentId) ??
			null
	);
	const dayLabel = $derived(
		selectedDay ? formatDayDate(selectedDay.examDate, selectedDay.label) : 'เลือกวันสอบ'
	);
	const workloadRows = $derived(
		[...(workspace?.staffWorkloads ?? [])].sort((a, b) => {
			const minutesCompare = b.totalMinutes - a.totalMinutes;
			return minutesCompare === 0
				? workloadStaffName(a).localeCompare(workloadStaffName(b), 'th')
				: minutesCompare;
		})
	);
	const selectedDayWorkloadRows = $derived(
		workloadRows.filter((workload) => selectedDayMinutes(workload) > 0)
	);
	const displayedStaffOptions = $derived(buildStaffOptions());

	function formatDayDate(value: string, label?: string | null): string {
		const dateLabel = new Date(value).toLocaleDateString('th-TH', {
			weekday: 'short',
			month: 'short',
			day: 'numeric'
		});
		return label ? `${label} · ${dateLabel}` : dateLabel;
	}

	function formatMinutes(minutes: number): string {
		const hours = Math.floor(minutes / 60);
		const remainder = minutes % 60;
		if (hours === 0) return `${remainder} นาที`;
		if (remainder === 0) return `${hours} ชม.`;
		return `${hours} ชม. ${remainder} นาที`;
	}

	function staffDisplayName(staffItem: ExamInvigilatorStaffOption): string {
		return staffItem.displayName || staffItem.staffId;
	}

	function workloadStaffName(workload: ExamInvigilatorStaffWorkload): string {
		return workload.staffName || workload.staffId;
	}

	function selectedDayMinutes(workload: ExamInvigilatorStaffWorkload): number {
		return workload.days.find((day) => day.examDayId === selectedDayId)?.minutes ?? 0;
	}

	function selectedDayAssignmentCount(workload: ExamInvigilatorStaffWorkload): number {
		return workload.days.find((day) => day.examDayId === selectedDayId)?.assignmentCount ?? 0;
	}

	function invigilatorNames(invigilators: InvigilatorView[]): string {
		if (invigilators.length === 0) return 'ยังไม่กำหนด';
		return invigilators
			.map((invigilator) => invigilator.displayName || invigilator.staffId)
			.join(', ');
	}

	function addStaffOption(options: Map<string, StaffOption>, option: StaffOption) {
		if (!options.has(option.id)) {
			options.set(option.id, option);
		}
	}

	function addStaffListItem(options: Map<string, StaffOption>, staffItem: ExamInvigilatorStaffOption) {
		addStaffOption(options, {
			id: staffItem.staffId,
			displayName: staffDisplayName(staffItem)
		});
	}

	function addInvigilatorOption(options: Map<string, StaffOption>, invigilator: InvigilatorView) {
		addStaffOption(options, {
			id: invigilator.staffId,
			displayName: invigilator.displayName || invigilator.staffId
		});
	}

	function buildStaffOptions(): StaffOption[] {
		const options = new Map<string, StaffOption>();

		for (const staffId of selectedStaffIds) {
			const cachedStaff =
				selectedStaffCache.find((item) => item.staffId === staffId) ??
				staffOptions.find((item) => item.staffId === staffId) ??
				staff.find((item) => item.staffId === staffId);
			const currentInvigilator = selectedAssignment?.invigilators.find(
				(invigilator) => invigilator.staffId === staffId
			);

			if (cachedStaff) {
				addStaffListItem(options, cachedStaff);
			} else if (currentInvigilator) {
				addInvigilatorOption(options, currentInvigilator);
			} else {
				addStaffOption(options, { id: staffId, displayName: staffId });
			}
		}

		for (const invigilator of selectedAssignment?.invigilators ?? []) {
			addInvigilatorOption(options, invigilator);
		}

		for (const staffItem of staffOptions) {
			addStaffListItem(options, staffItem);
		}

		return [...options.values()].sort((a, b) => {
			const aSelected = selectedStaffIds.includes(a.id);
			const bSelected = selectedStaffIds.includes(b.id);
			if (aSelected !== bSelected) return aSelected ? -1 : 1;
			return a.displayName.localeCompare(b.displayName, 'th');
		});
	}

	function loadAssignment(assignment: ExamInvigilatorAssignmentSummary) {
		selectedAssignmentId = assignment.assignmentId;
		selectedStaffIds = assignment.invigilators.map((invigilator) => invigilator.staffId);
		selectedStaffCache = staff.filter((staffItem) => selectedStaffIds.includes(staffItem.staffId));
		resetStaffSearch();
		editorOpen = true;
	}

	function cancelStaffSearchRequest() {
		staffSearchRequestToken += 1;
		staffSearchLoading = false;
	}

	function syncDefaultStaffOptions() {
		if (staffOptions !== staff) {
			staffOptions = staff;
		}
		if (staffSearchLoading) {
			staffSearchLoading = false;
		}
		if (staffSearchError) {
			staffSearchError = '';
		}
	}

	function resetStaffSearch() {
		cancelStaffSearchRequest();
		staffSearch = '';
		syncDefaultStaffOptions();
	}

	function toggleStaff(staffId: string, checked: boolean) {
		if (readonly) return;

		if (checked) {
			if (!selectedStaffIds.includes(staffId)) {
				selectedStaffIds = [...selectedStaffIds, staffId];
			}

			const staffItem =
				staffOptions.find((item) => item.staffId === staffId) ??
				staff.find((item) => item.staffId === staffId);
			if (staffItem && !selectedStaffCache.some((item) => item.staffId === staffItem.staffId)) {
				selectedStaffCache = [...selectedStaffCache, staffItem];
			}
		} else {
			selectedStaffIds = selectedStaffIds.filter((id) => id !== staffId);
		}
	}

	async function submitInvigilators() {
		if (!selectedAssignmentId || readonly) return;

		const saved = await onSaveInvigilators?.(selectedAssignmentId, selectedStaffIds);
		if (saved) {
			editorOpen = false;
		}
	}

	$effect(() => {
		if (!selectedDayId && sortedDays[0]) {
			selectedDayId = sortedDays[0].id;
		}
		if (selectedDayId && !days.some((day) => day.id === selectedDayId)) {
			selectedDayId = sortedDays[0]?.id ?? '';
			editorOpen = false;
			selectedAssignmentId = null;
			selectedStaffIds = [];
		}
	});

	$effect(() => {
		if (
			selectedAssignmentId &&
			!workspace?.assignments.some((assignment) => assignment.assignmentId === selectedAssignmentId)
		) {
			editorOpen = false;
			selectedAssignmentId = null;
			selectedStaffIds = [];
		}
	});

	$effect(() => {
		const search = staffSearch.trim();

		if (!editorOpen || readonly || !onSearchStaff || !search) {
			syncDefaultStaffOptions();
			return;
		}

		const requestToken = ++staffSearchRequestToken;
		staffSearchLoading = true;
		staffSearchError = '';

		const timer = setTimeout(async () => {
			try {
				const results = await onSearchStaff(search);
				if (requestToken === staffSearchRequestToken) {
					staffOptions = results;
				}
			} catch (searchError) {
				if (requestToken === staffSearchRequestToken) {
					staffSearchError =
						searchError instanceof Error ? searchError.message : 'ค้นหาครูไม่สำเร็จ';
				}
			} finally {
				if (requestToken === staffSearchRequestToken) {
					staffSearchLoading = false;
				}
			}
		}, 300);

		return () => {
			clearTimeout(timer);
			if (requestToken === staffSearchRequestToken) {
				cancelStaffSearchRequest();
			}
		};
	});
</script>

{#if loadError}
	<section class="rounded-md border bg-background">
		<PageState
			variant="error"
			title="โหลดข้อมูลกรรมการคุมสอบไม่สำเร็จ"
			description={loadError}
			actionLabel="ลองอีกครั้ง"
			onaction={onRetry}
		/>
	</section>
{:else if workspace === null}
	<section class="rounded-md border bg-background">
		<PageState
			title={loading ? 'กำลังโหลดข้อมูลกรรมการคุมสอบ' : 'ยังไม่มีข้อมูลกรรมการคุมสอบ'}
			description="ข้อมูลอ้างอิงจากห้องสอบที่กำหนดไว้ในรอบนี้"
		/>
	</section>
{:else}
	<section class="overflow-hidden rounded-md border bg-background">
		<div
			class="flex flex-col gap-3 border-b px-4 py-4 lg:flex-row lg:items-center lg:justify-between"
		>
			<div>
				<h2 class="font-semibold">กรรมการคุมสอบตามห้องสอบ</h2>
				<p class="text-sm text-muted-foreground">
					{selectedDayAssignments.length} ห้องในวันที่เลือก
				</p>
			</div>
			<Select.Root type="single" bind:value={selectedDayId}>
				<Select.Trigger class="w-full sm:w-64">{dayLabel}</Select.Trigger>
				<Select.Content>
					{#each sortedDays as day (day.id)}
						<Select.Item value={day.id}>{formatDayDate(day.examDate, day.label)}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		<div class="grid gap-0 xl:grid-cols-[minmax(0,1fr)_24rem]">
			<div class="min-w-0 border-b xl:border-b-0 xl:border-r">
				{#if !selectedDay}
					<PageState title="ยังไม่มีวันสอบ" description="ต้องมีวันสอบก่อนจัดกรรมการคุมสอบ" />
				{:else if selectedDayAssignments.length === 0}
					<PageState
						title="ยังไม่มีห้องสอบในวันนี้"
						description="กำหนดห้องสอบในแท็บ Rooms ก่อนจัดกรรมการ"
					/>
				{:else}
					<div class="overflow-x-auto">
						<Table class="min-w-[760px]">
							<TableHeader>
								<TableRow>
									<TableHead>ห้องเรียน</TableHead>
									<TableHead>ห้องสอบ</TableHead>
									<TableHead>กรรมการ</TableHead>
									<TableHead class="w-32 text-center">เวลาคุมสอบ</TableHead>
									<TableHead class="w-32 text-right">จัดการ</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{#each selectedDayAssignments as assignment (assignment.assignmentId)}
									<TableRow>
										<TableCell class="font-medium">{assignment.classroomName || '-'}</TableCell>
										<TableCell>{assignment.roomName || '-'}</TableCell>
										<TableCell>
											<div class="flex flex-col gap-1">
												<span>{invigilatorNames(assignment.invigilators)}</span>
												<div class="flex flex-wrap gap-1">
													<Badge variant="outline">{assignment.invigilators.length} คน</Badge>
													{#if assignment.invigilators.length < 2}
														<Badge variant="outline">แนะนำ 2 คน</Badge>
													{/if}
												</div>
											</div>
										</TableCell>
										<TableCell class="text-center">
											<Badge variant="outline">{formatMinutes(assignment.sessionMinutes)}</Badge>
										</TableCell>
										<TableCell class="text-right">
											{#if readonly}
												<Badge variant="outline">อ่านอย่างเดียว</Badge>
											{:else}
												<LoadingButton
													variant="outline"
													size="sm"
													loading={savingAssignmentId === assignment.assignmentId}
													loadingLabel="กำลังบันทึก..."
													onclick={() => loadAssignment(assignment)}
												>
													<UserRoundPlus class="h-4 w-4" />
													จัดกรรมการ
												</LoadingButton>
											{/if}
										</TableCell>
									</TableRow>
								{/each}
							</TableBody>
						</Table>
					</div>
				{/if}
			</div>

			<aside class="space-y-4 p-4">
				<div class="space-y-2">
					<div>
						<h3 class="text-sm font-semibold">ภาระงานทั้งรอบ</h3>
						<p class="text-xs text-muted-foreground">{workloadRows.length} คนที่ถูกมอบหมาย</p>
					</div>
					{#if workloadRows.length === 0}
						<PageState title="ยังไม่มีภาระงาน" description="ยังไม่มีกรรมการคุมสอบในรอบนี้" />
					{:else}
						<div class="space-y-2">
							{#each workloadRows as workload (workload.staffId)}
								<div class="rounded-md border p-3 text-sm">
									<div class="flex items-center justify-between gap-3">
										<span class="font-medium">{workloadStaffName(workload)}</span>
										<Badge variant="outline">{formatMinutes(workload.totalMinutes)}</Badge>
									</div>
									<p class="mt-1 text-xs text-muted-foreground">
										{workload.assignedDayCount} วัน · {workload.assignmentCount} ห้อง
									</p>
								</div>
							{/each}
						</div>
					{/if}
				</div>

				<div class="space-y-2">
					<div>
						<h3 class="text-sm font-semibold">ภาระงานวันที่เลือก</h3>
						<p class="text-xs text-muted-foreground">{dayLabel}</p>
					</div>
					{#if selectedDayWorkloadRows.length === 0}
						<PageState
							title="ยังไม่มีภาระงานในวันนี้"
							description="ยังไม่มีกรรมการในวันสอบที่เลือก"
						/>
					{:else}
						<div class="space-y-2">
							{#each selectedDayWorkloadRows as workload (workload.staffId)}
								<div class="rounded-md border p-3 text-sm">
									<div class="flex items-center justify-between gap-3">
										<span class="font-medium">{workloadStaffName(workload)}</span>
										<Badge variant="outline">{formatMinutes(selectedDayMinutes(workload))}</Badge>
									</div>
									<p class="mt-1 text-xs text-muted-foreground">
										{selectedDayAssignmentCount(workload)} ห้องในวันนี้
									</p>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</aside>
		</div>
	</section>
{/if}

{#if !readonly}
	<Sheet.Root bind:open={editorOpen}>
		<Sheet.Content side="right" class="sm:max-w-lg">
			<Sheet.Header>
				<Sheet.Title>จัดกรรมการ</Sheet.Title>
				<Sheet.Description>
					{selectedAssignment?.classroomName ?? 'ห้องเรียน'} · {selectedAssignment?.roomName ??
						'ห้องสอบ'}
				</Sheet.Description>
			</Sheet.Header>

			<form
				class="flex min-h-0 flex-1 flex-col"
				onsubmit={(event) => {
					event.preventDefault();
					submitInvigilators();
				}}
			>
				<div class="flex-1 space-y-4 overflow-y-auto py-1 pr-1">
					<div class="flex flex-wrap items-center gap-2">
						<Badge variant="outline">{selectedStaffIds.length} คน</Badge>
						{#if selectedStaffIds.length < 2}
							<Badge variant="outline">แนะนำ 2 คน</Badge>
						{/if}
					</div>

					<div class="grid gap-2">
						<Label for="exam-invigilator-staff-search">ค้นหาครู</Label>
						<div class="relative">
							<Search
								class="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-muted-foreground"
							/>
							<Input
								id="exam-invigilator-staff-search"
									type="search"
									class="pl-9"
									bind:value={staffSearch}
									placeholder="ชื่อครู"
									disabled={savingAssignmentId === selectedAssignmentId}
								/>
						</div>
						{#if staffSearchLoading}
							<p class="text-xs text-muted-foreground">กำลังค้นหา...</p>
						{:else if staffSearchError}
							<p class="text-xs text-destructive">{staffSearchError}</p>
						{/if}
					</div>

					<div class="space-y-2">
						<Label>รายชื่อครู</Label>
						{#if displayedStaffOptions.length === 0}
							<PageState title="ไม่พบรายชื่อครู" description="ลองค้นหาด้วยคำอื่น" />
						{:else}
							<div class="grid max-h-[28rem] gap-2 overflow-y-auto rounded-md border p-3">
								{#each displayedStaffOptions as option (option.id)}
									<label
										class="flex items-start gap-3 rounded-md px-2 py-1.5 text-sm hover:bg-muted/60"
									>
										<Checkbox
											class="mt-0.5"
											checked={selectedStaffIds.includes(option.id)}
											disabled={savingAssignmentId === selectedAssignmentId}
											onCheckedChange={(checked) => toggleStaff(option.id, checked === true)}
										/>
										<span class="min-w-0">
											<span class="block truncate font-medium">{option.displayName}</span>
											{#if option.detail}
												<span class="block truncate text-xs text-muted-foreground"
													>{option.detail}</span
												>
											{/if}
										</span>
									</label>
								{/each}
							</div>
						{/if}
					</div>
				</div>

				<Sheet.Footer class="mt-auto gap-2 pt-4">
					<Button type="button" variant="outline" onclick={() => (editorOpen = false)}>
						ยกเลิก
					</Button>
					<LoadingButton
						type="submit"
						loading={savingAssignmentId === selectedAssignmentId}
						loadingLabel="กำลังบันทึก..."
						disabled={!onSaveInvigilators}
					>
						บันทึกกรรมการ
					</LoadingButton>
				</Sheet.Footer>
			</form>
		</Sheet.Content>
	</Sheet.Root>
{/if}
