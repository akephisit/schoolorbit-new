<script lang="ts">
	import { addMonths } from 'date-fns';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import { Separator } from '$lib/components/ui/separator';
	import CalendarMonthGrid from '$lib/components/calendar/CalendarMonthGrid.svelte';
	import CalendarEventList from '$lib/components/calendar/CalendarEventList.svelte';
	import CalendarEventDialog from '$lib/components/calendar/CalendarEventDialog.svelte';
	import CalendarCategoryDialog from '$lib/components/calendar/CalendarCategoryDialog.svelte';
	import {
		type CalendarAudienceType,
		type CalendarCategory,
		type CalendarEvent,
		type CreateCalendarEventRequest,
		type UpsertCalendarCategoryRequest,
		createCalendarCategory,
		createCalendarEvent,
		deleteCalendarCategory,
		deleteCalendarEvent,
		listCalendarCategories,
		listCalendarEvents,
		updateCalendarCategory,
		updateCalendarEvent
	} from '$lib/api/calendar';
	import { getAcademicStructure, listClassrooms } from '$lib/api/academic';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		calendarGridRange,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '$lib/utils/calendar';
	import {
		CalendarDays,
		ChevronLeft,
		ChevronRight,
		FolderPlus,
		Plus,
		RefreshCw,
		Search,
		SlidersHorizontal
	} from 'lucide-svelte';

	type GradeLevelOption = { id: string; name: string };
	type ClassroomOption = { id: string; name: string; grade_level_id?: string };
	type VisibilityFilter = '' | 'public' | 'private';
	type AudienceFilter = '' | CalendarAudienceType;

	let { data } = $props();

	const todayDate = toIsoDate(new Date());

	let events = $state.raw<CalendarEvent[]>([]);
	let categories = $state.raw<CalendarCategory[]>([]);
	let loading = $state(true);
	let selectedMonth = $state(todayDate);
	let selectedDate = $state(todayDate);
	let search = $state('');
	let categoryId = $state('');
	let audience = $state<AudienceFilter>('');
	let visibility = $state<VisibilityFilter>('');
	let eventDialogOpen = $state(false);
	let categoryDialogOpen = $state(false);
	let editingEvent = $state<CalendarEvent | null>(null);
	let saving = $state(false);
	let error = $state('');
	let gradeLevels = $state.raw<GradeLevelOption[]>([]);
	let classrooms = $state.raw<ClassroomOption[]>([]);
	let manageOptionsLoaded = $state(false);
	let manageOptionsLoading = $state(false);
	let manageOptionsPromise = $state<Promise<boolean> | null>(null);
	let deleteDialogOpen = $state(false);
	let deletingEvent = $state<CalendarEvent | null>(null);
	let deleting = $state(false);
	let calendarLoadSequence = 0;

	const audienceOptions: { value: AudienceFilter; label: string }[] = [
		{ value: '', label: 'ทุกกลุ่มผู้ชม' },
		{ value: 'all', label: 'ทุกคน' },
		{ value: 'staff', label: 'บุคลากร' },
		{ value: 'student', label: 'นักเรียน' },
		{ value: 'parent', label: 'ผู้ปกครอง' }
	];
	const visibilityOptions: { value: VisibilityFilter; label: string }[] = [
		{ value: '', label: 'ทุกสถานะ' },
		{ value: 'public', label: 'สาธารณะ' },
		{ value: 'private', label: 'ภายในโรงเรียน' }
	];

	const canReadCalendar = $derived($can.has(PERMISSIONS.CALENDAR_READ_SCHOOL));
	const canManageCalendar = $derived($can.has(PERMISSIONS.CALENDAR_MANAGE_SCHOOL));
	const activeCategories = $derived(categories.filter((category) => category.isActive));
	const monthLabel = $derived(formatCalendarMonth(selectedMonth));
	const selectedDateEvents = $derived(
		events
			.filter((event) => eventOverlapsDate(event, selectedDate))
			.sort(
				(left, right) =>
					left.startDate.localeCompare(right.startDate) ||
					(left.startTime ?? '').localeCompare(right.startTime ?? '') ||
					left.title.localeCompare(right.title)
			)
	);
	const selectedMonthEvents = $derived.by(() => {
		const range = monthRange(selectedMonth);
		return events.filter((event) => event.startDate <= range.to && event.endDate >= range.from);
	});
	const publicEventCount = $derived(selectedMonthEvents.filter((event) => event.isPublic).length);
	const activeFilterCount = $derived(
		[search.trim(), categoryId, audience, visibility].filter(Boolean).length
	);
	const isTodaySelected = $derived(selectedDate === todayDate);
	const categoryLabel = $derived(
		activeCategories.find((category) => category.id === categoryId)?.name ?? 'ทุกหมวดหมู่'
	);
	const audienceLabel = $derived(
		audienceOptions.find((option) => option.value === audience)?.label
	);
	const visibilityLabel = $derived(
		visibilityOptions.find((option) => option.value === visibility)?.label
	);

	async function loadCalendar() {
		if (!canReadCalendar) {
			loading = false;
			error = '';
			return;
		}

		const requestSequence = ++calendarLoadSequence;
		loading = true;
		error = '';

		try {
			const range = calendarGridRange(selectedMonth);
			const [nextEvents, nextCategories] = await Promise.all([
				listCalendarEvents({
					...range,
					categoryId: categoryId || undefined,
					audience: audience || undefined,
					visibility: visibility || undefined,
					q: search.trim() || undefined
				}),
				listCalendarCategories()
			]);

			if (requestSequence !== calendarLoadSequence) return;
			events = nextEvents;
			categories = nextCategories;
		} catch (loadError: unknown) {
			if (requestSequence !== calendarLoadSequence) return;
			error =
				(loadError instanceof Error ? loadError.message : String(loadError)) ||
				'โหลดปฏิทินไม่สำเร็จ';
			toast.error(error);
		} finally {
			if (requestSequence === calendarLoadSequence) {
				loading = false;
			}
		}
	}

	function sortCalendarEvents(items: CalendarEvent[]) {
		return [...items].sort(
			(left, right) =>
				left.startDate.localeCompare(right.startDate) || left.title.localeCompare(right.title)
		);
	}

	function eventMatchesCurrentFilters(event: CalendarEvent) {
		const range = calendarGridRange(selectedMonth);
		if (event.startDate > range.to || event.endDate < range.from) return false;
		if (categoryId && event.categoryId !== categoryId) return false;
		if (visibility === 'public' && !event.isPublic) return false;
		if (visibility === 'private' && event.isPublic) return false;
		if (audience && !event.targets.some((target) => target.audienceType === audience)) {
			return false;
		}

		const query = search.trim().toLowerCase();
		if (query) {
			const searchableText = [event.title, event.description ?? '', event.location ?? '']
				.join(' ')
				.toLowerCase();
			if (!searchableText.includes(query)) return false;
		}

		return true;
	}

	function patchSavedEvent(event: CalendarEvent) {
		if (!eventMatchesCurrentFilters(event)) {
			events = events.filter((item) => item.id !== event.id);
			return;
		}

		events = sortCalendarEvents(
			events.some((item) => item.id === event.id)
				? events.map((item) => (item.id === event.id ? event : item))
				: [event, ...events]
		);
	}

	async function saveEvent(payload: CreateCalendarEventRequest) {
		saving = true;
		try {
			const savedEvent = editingEvent
				? await updateCalendarEvent(editingEvent.id, payload)
				: await createCalendarEvent(payload);
			patchSavedEvent(savedEvent);
			eventDialogOpen = false;
			editingEvent = null;
			toast.success('บันทึกกิจกรรมแล้ว');
		} catch (saveError: unknown) {
			toast.error(
				(saveError instanceof Error ? saveError.message : String(saveError)) || 'บันทึกไม่สำเร็จ'
			);
		} finally {
			saving = false;
		}
	}

	function requestDeleteEvent(event: { id: string }) {
		const target = events.find((item) => item.id === event.id);
		if (!target || !canManageCalendar) return;
		deletingEvent = target;
		deleteDialogOpen = true;
	}

	function cancelDeleteEvent() {
		if (deleting) return;
		deleteDialogOpen = false;
		deletingEvent = null;
	}

	async function confirmDeleteEvent() {
		const target = deletingEvent;
		if (!target || !canManageCalendar) return;

		deleting = true;
		try {
			await deleteCalendarEvent(target.id);
			events = events.filter((item) => item.id !== target.id);
			deleteDialogOpen = false;
			deletingEvent = null;
			toast.success('ลบกิจกรรมแล้ว');
		} catch (deleteError: unknown) {
			deleteDialogOpen = true;
			toast.error(
				(deleteError instanceof Error ? deleteError.message : String(deleteError)) ||
					'ลบกิจกรรมไม่สำเร็จ'
			);
		} finally {
			deleting = false;
		}
	}

	async function saveCategory(id: string | null, payload: UpsertCalendarCategoryRequest) {
		saving = true;
		try {
			const savedCategory = id
				? await updateCalendarCategory(id, payload)
				: await createCalendarCategory(payload);
			categories = categories.some((category) => category.id === savedCategory.id)
				? categories.map((category) =>
						category.id === savedCategory.id ? savedCategory : category
					)
				: [...categories, savedCategory];
			events = sortCalendarEvents(
				events
					.map((event) =>
						event.categoryId === savedCategory.id
							? {
									...event,
									categoryName: savedCategory.name,
									categoryColor: savedCategory.color
								}
							: event
					)
					.filter(eventMatchesCurrentFilters)
			);
			categoryDialogOpen = false;
			toast.success('บันทึกหมวดหมู่แล้ว');
		} catch (saveError: unknown) {
			toast.error(
				(saveError instanceof Error ? saveError.message : String(saveError)) ||
					'บันทึกหมวดหมู่ไม่สำเร็จ'
			);
		} finally {
			saving = false;
		}
	}

	async function deactivateCategory(category: CalendarCategory) {
		if (!canManageCalendar) return;

		saving = true;
		try {
			await deleteCalendarCategory(category.id);
			categories = categories.map((item) =>
				item.id === category.id ? { ...item, isActive: false } : item
			);
			if (categoryId === category.id) {
				categoryId = '';
				await loadCalendar();
			}
			toast.success('ปิดใช้งานหมวดหมู่แล้ว');
		} catch (deleteError: unknown) {
			toast.error(
				(deleteError instanceof Error ? deleteError.message : String(deleteError)) ||
					'ปิดใช้งานหมวดหมู่ไม่สำเร็จ'
			);
		} finally {
			saving = false;
		}
	}

	async function ensureManageOptions(): Promise<boolean> {
		if (manageOptionsLoaded) return true;
		if (manageOptionsPromise) return manageOptionsPromise;

		manageOptionsLoading = true;
		manageOptionsPromise = (async () => {
			try {
				const structure = await getAcademicStructure();
				const activeYear =
					structure.data.years.find((year) => year.is_active) ?? structure.data.years[0];
				const classroomsResponse = activeYear
					? await listClassrooms({ year_id: activeYear.id })
					: null;

				gradeLevels = structure.data.levels
					.filter((level) => level.is_active)
					.map((level) => ({ id: level.id, name: level.short_name || level.name }));
				classrooms =
					classroomsResponse?.data
						.filter((classroom) => classroom.is_active)
						.map((classroom) => ({
							id: classroom.id,
							name: classroom.name,
							grade_level_id: classroom.grade_level_id
						})) ?? [];
				manageOptionsLoaded = true;
				return true;
			} catch (loadError: unknown) {
				toast.error(
					(loadError instanceof Error ? loadError.message : String(loadError)) ||
						'โหลดตัวเลือกชั้นเรียนไม่สำเร็จ'
				);
				return false;
			} finally {
				manageOptionsLoading = false;
				manageOptionsPromise = null;
			}
		})();

		return manageOptionsPromise;
	}

	async function openEventDialog(event: { id: string } | null = null) {
		if (!canManageCalendar) return;

		const optionsReady = await ensureManageOptions();
		if (!optionsReady) return;

		editingEvent = event ? (events.find((item) => item.id === event.id) ?? null) : null;
		eventDialogOpen = true;
	}

	function openCategoryDialog() {
		if (!canManageCalendar) return;
		categoryDialogOpen = true;
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
		selectedMonth = todayDate;
		selectedDate = todayDate;
		await loadCalendar();
	}

	async function resetFilters() {
		search = '';
		categoryId = '';
		audience = '';
		visibility = '';
		await loadCalendar();
	}

	onMount(() => {
		void loadCalendar();
	});
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<PageShell title="ปฏิทินโรงเรียน" description="กิจกรรมและประกาศตามช่วงเดือน">
	{#snippet actions()}
		<div class="flex flex-wrap gap-2">
			{#if canManageCalendar}
				<Button variant="outline" onclick={openCategoryDialog}>
					<FolderPlus class="h-4 w-4" />
					หมวดหมู่
				</Button>
				<Button onclick={() => openEventDialog()} disabled={manageOptionsLoading}>
					<Plus class="h-4 w-4" />
					เพิ่มกิจกรรม
				</Button>
			{/if}
		</div>
	{/snippet}

	{#if canReadCalendar || loading}
		<div class="overflow-hidden rounded-xl border bg-card shadow-sm">
			<div class="flex flex-col gap-3 p-3 sm:flex-row sm:items-center sm:justify-between sm:p-4">
				<div class="flex min-w-0 items-center gap-1 sm:gap-2">
					<Button
						variant="outline"
						size="icon"
						onclick={() => changeMonth(-1)}
						aria-label="เดือนก่อนหน้า"
					>
						<ChevronLeft class="size-4" />
					</Button>
					<div class="min-w-0 flex-1 px-2 sm:min-w-52 sm:flex-none">
						<div class="flex items-center gap-2">
							<CalendarDays class="size-4 shrink-0 text-primary" />
							<h2 class="truncate text-base font-semibold capitalize">{monthLabel}</h2>
						</div>
						<p class="mt-0.5 text-xs text-muted-foreground">
							{selectedMonthEvents.length} กิจกรรม · {publicEventCount} สาธารณะ
						</p>
					</div>
					<Button
						variant="outline"
						size="icon"
						onclick={() => changeMonth(1)}
						aria-label="เดือนถัดไป"
					>
						<ChevronRight class="size-4" />
					</Button>
				</div>

				<div class="flex items-center gap-2">
					<Button variant="outline" size="sm" onclick={goToToday} disabled={isTodaySelected}>
						วันนี้
					</Button>
					<Button
						variant="ghost"
						size="icon"
						onclick={loadCalendar}
						disabled={loading}
						aria-label="รีเฟรชปฏิทิน"
					>
						<RefreshCw class={loading ? 'size-4 animate-spin' : 'size-4'} />
					</Button>
				</div>
			</div>

			<Separator />

			<form
				class="grid gap-3 p-3 sm:grid-cols-2 sm:p-4 xl:grid-cols-[minmax(240px,1fr)_180px_180px_180px_auto]"
				onsubmit={(submitEvent) => {
					submitEvent.preventDefault();
					loadCalendar();
				}}
			>
				<div class="relative sm:col-span-2 xl:col-span-1">
					<Search class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
					<Input class="pl-9" placeholder="ค้นหาชื่อ รายละเอียด หรือสถานที่" bind:value={search} />
				</div>
				<Select.Root type="single" bind:value={categoryId}>
					<Select.Trigger class="w-full">{categoryLabel}</Select.Trigger>
					<Select.Content>
						<Select.Item value="">ทุกหมวดหมู่</Select.Item>
						{#each activeCategories as category (category.id)}
							<Select.Item value={category.id}>{category.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				<Select.Root type="single" bind:value={audience}>
					<Select.Trigger class="w-full">{audienceLabel}</Select.Trigger>
					<Select.Content>
						{#each audienceOptions as option (option.value)}
							<Select.Item value={option.value}>{option.label}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				<Select.Root type="single" bind:value={visibility}>
					<Select.Trigger class="w-full">{visibilityLabel}</Select.Trigger>
					<Select.Content>
						{#each visibilityOptions as option (option.value)}
							<Select.Item value={option.value}>{option.label}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				<div class="flex items-center gap-2 sm:col-span-2 xl:col-span-1">
					<Button type="submit" class="flex-1 xl:flex-none">
						<SlidersHorizontal class="size-4" />
						กรอง
						{#if activeFilterCount > 0}
							<Badge variant="secondary" class="ml-1 min-w-5 justify-center px-1">
								{activeFilterCount}
							</Badge>
						{/if}
					</Button>
					{#if activeFilterCount > 0}
						<Button type="button" variant="ghost" onclick={resetFilters}>ล้างตัวกรอง</Button>
					{/if}
				</div>
			</form>
		</div>
	{/if}

	{#if !canReadCalendar && !loading}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูปฏิทินโรงเรียน"
			description="ติดต่อผู้ดูแลระบบหากต้องการเข้าถึงข้อมูลนี้"
		/>
	{:else if loading}
		<PageSkeleton variant="detail" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดปฏิทินไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadCalendar}
		/>
	{:else}
		<div class="grid items-start gap-5 xl:grid-cols-[minmax(0,1fr)_400px]">
			<div class="min-w-0 space-y-3">
				<CalendarMonthGrid
					monthDate={selectedMonth}
					{events}
					{selectedDate}
					onselect={(date) => (selectedDate = date)}
				/>
				{#if activeCategories.length > 0}
					<div
						class="flex flex-wrap items-center gap-x-4 gap-y-2 px-1 text-xs text-muted-foreground"
					>
						{#each activeCategories as category (category.id)}
							<span class="flex items-center gap-1.5">
								<span
									class="size-2 rounded-full"
									style:background-color={category.color}
									aria-hidden="true"
								></span>
								{category.name}
							</span>
						{/each}
					</div>
				{/if}
			</div>
			<section class="space-y-3">
				<div class="flex items-center gap-3 rounded-xl border bg-card p-4 shadow-sm">
					<div
						class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary"
					>
						<CalendarDays class="size-5" />
					</div>
					<div class="min-w-0">
						<h2 class="truncate font-semibold">{formatCalendarDate(selectedDate)}</h2>
						<p class="text-sm text-muted-foreground">
							{selectedDateEvents.length} รายการในวันที่เลือก
						</p>
					</div>
				</div>
				<CalendarEventList
					events={selectedDateEvents}
					canManage={canManageCalendar}
					onedit={openEventDialog}
					ondelete={requestDeleteEvent}
				/>
			</section>
		</div>
	{/if}

	<AlertDialog.Root bind:open={deleteDialogOpen}>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>ลบกิจกรรมนี้หรือไม่</AlertDialog.Title>
				<AlertDialog.Description>
					กิจกรรม “{deletingEvent?.title ?? ''}” จะหายจากปฏิทินของผู้ใช้งานทุกกลุ่ม
					และยกเลิกการแจ้งเตือนที่ยังไม่ถูกส่ง
				</AlertDialog.Description>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Cancel disabled={deleting} onclick={cancelDeleteEvent}
					>ยกเลิก</AlertDialog.Cancel
				>
				<AlertDialog.Action variant="destructive" disabled={deleting} onclick={confirmDeleteEvent}>
					{deleting ? 'กำลังลบ...' : 'ลบกิจกรรม'}
				</AlertDialog.Action>
			</AlertDialog.Footer>
		</AlertDialog.Content>
	</AlertDialog.Root>

	<CalendarEventDialog
		bind:open={eventDialogOpen}
		{categories}
		{gradeLevels}
		{classrooms}
		event={editingEvent}
		{saving}
		onsave={saveEvent}
	/>
	<CalendarCategoryDialog
		bind:open={categoryDialogOpen}
		{categories}
		{saving}
		onsave={saveCategory}
		ondeactivate={deactivateCategory}
	/>
</PageShell>
