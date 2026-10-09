<script lang="ts">
	import { page } from '$app/state';
	import { addMonths } from 'date-fns';
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { calendarRouteFilters } from '#lib/utils/calendar-route-filters.js';
	import { authStore } from '#lib/stores/auth.js';
	import { Skeleton } from '#lib/components/ui/skeleton/index.js';
	import { toast } from 'svelte-sonner';

	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import CalendarCreateMenu from '#lib/components/calendar/CalendarCreateMenu.svelte';
	import CalendarMonthGrid, {
		type CalendarDisplayEvent
	} from '#lib/components/calendar/CalendarMonthGrid.svelte';
	import CalendarEventList from '#lib/components/calendar/CalendarEventList.svelte';
	import CalendarEventDialog from '#lib/components/calendar/CalendarEventDialog.svelte';
	import CalendarCategoryDialog from '#lib/components/calendar/CalendarCategoryDialog.svelte';
	import CalendarEmbedDialog from '#lib/components/calendar/CalendarEmbedDialog.svelte';
	import CalendarRequestDialog from '#lib/components/calendar/CalendarRequestDialog.svelte';
	import CalendarRequestReviewDialog from '#lib/components/calendar/CalendarRequestReviewDialog.svelte';
	import CalendarColorKey from '#lib/components/calendar/CalendarColorKey.svelte';
	import CalendarSearchDialog from '#lib/components/calendar/CalendarSearchDialog.svelte';
	import {
		type CalendarAudienceType,
		type CalendarTargetOptions,
		type CreateCalendarRequest,
		type PendingCalendarRequest,
		type CalendarEventRequest,
		listPendingCalendarRequests,
		createCalendarRequest,
		listCalendarTargetOptions,
		type CalendarCategory,
		type CalendarEvent,
		type CalendarTag,
		type CreateCalendarEventRequest,
		type UpsertCalendarCategoryRequest,
		type UpsertCalendarTagRequest,
		createCalendarCategory,
		createCalendarEvent,
		createCalendarTag,
		deleteCalendarCategory,
		deleteCalendarEvent,
		deleteCalendarTag,
		listCalendarCategories,
		listCalendarEvents,
		listCalendarTags,
		updateCalendarCategory,
		updateCalendarEvent,
		updateCalendarTag
	} from '#lib/api/calendar.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import {
		calendarGridRange,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '#lib/utils/calendar.js';
	import {
		CalendarDays,
		ClipboardList,
		ChevronLeft,
		ChevronRight,
		Code2,
		Copy,
		FolderPlus,
		Settings,
		RefreshCw,
		Search,
		SlidersHorizontal,
		X
	} from '@lucide/svelte';

	type VisibilityFilter = '' | 'public' | 'private';
	type AudienceFilter = '' | CalendarAudienceType;

	const todayDate = toIsoDate(new Date());
	const currentUrl = $derived(
		page.state.calendarUrl ? new URL(page.state.calendarUrl) : new URL(page.url.href)
	);
	const committed = $derived(calendarRouteFilters(currentUrl));
	let { data }: PageProps = $props();
	const eventsSource = $derived(data.events),
		categoriesSource = $derived(data.categories),
		tagsSource = $derived(data.tags);

	let events = $state.raw<CalendarEvent[]>([]);
	let categories = $state.raw<CalendarCategory[]>([]);
	let tags = $state.raw<CalendarTag[]>([]);
	let loading = $state(true);
	const selectedMonth = $derived(committed.month);
	let selectedDate = $state(todayDate);
	let detailOpen = $state(false);
	let detailKind = $state<'day' | 'event' | 'request'>('day');
	let detailId = $state('');
	let detailAnchor = $state.raw<HTMLElement | null>(null);
	let search = $state('');
	let searchOpen = $state(false);
	let categoryId = $state('');
	let tagId = $state('');
	let audience = $state<AudienceFilter>('');
	let visibility = $state<VisibilityFilter>('');
	let eventDialogOpen = $state(false);
	let requestDialogOpen = $state(false);
	let requesting = $state(false);
	let requestSession = $state(0);
	let requestError = $state('');
	let showPendingRequests = $state(false),
		filterOpen = $state(false);
	let reviewIdentity = $state('');
	let reviewTarget = $state<PendingCalendarRequest | null>(null),
		reviewOpen = $state(false),
		reviewMode = $state<'approve' | 'reject'>('approve'),
		reviewSession = $state(0);
	let pendingRequests = $state.raw<PendingCalendarRequest[]>([]);
	let pendingLoading = $state(false),
		pendingLoaded = $state(false),
		pendingHasMore = $state(false),
		pendingError = $state('');
	const pendingRequest = new LatestRequest();
	let pendingOwner = '';
	let optionsDate = $state('');
	let eventDialogSession = $state(0);
	let categoryDialogOpen = $state(false);
	let embedDialogOpen = $state(false);
	let editingEvent = $state<CalendarEvent | null>(null);
	let saving = $state(false);
	let error = $state('');
	let gradeLevels = $state.raw<CalendarTargetOptions['gradeLevels']>([]);
	let homerooms = $state.raw<CalendarTargetOptions['homerooms']>([]);
	let manageOptionsLoaded = $state(false);
	let manageOptionsLoading = $state(false);
	let deleteDialogOpen = $state(false);
	let deletingEvent = $state<CalendarEvent | null>(null);
	let deleting = $state(false);
	let calendarRendered = $state(false);
	let dismissDetailClick = false;
	let eventsLoaded = $state(false),
		categoriesLoaded = $state(false),
		categoriesLoading = $state(true),
		categoriesError = $state(''),
		tagsLoaded = $state(false),
		tagsLoading = $state(true),
		tagsError = $state('');
	let manageOptionsError = $state('');
	const eventsRequest = new LatestRequest(),
		categoriesRequest = new LatestRequest(),
		tagsRequest = new LatestRequest(),
		optionsRequest = new LatestRequest();
	let consumedEventsSource: typeof data.events | null = null;
	let activeEventOwner = '',
		identityOwner = '',
		ownerEpoch = 0,
		eventDraft = 0,
		categoryDraft = 0,
		disposed = false,
		actionSequence = 0;

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
	const canRequestCalendar = $derived(
		canReadCalendar && $can.has(PERMISSIONS.CALENDAR_REQUEST_OWN)
	);
	const canManageCalendar = $derived($can.has(PERMISSIONS.CALENDAR_MANAGE_SCHOOL));
	const canViewPendingRequests = $derived(
		canReadCalendar && (canRequestCalendar || canManageCalendar)
	);
	const pendingDisplayEvents = $derived(
		showPendingRequests && canViewPendingRequests
			? pendingRequests.map((request) => ({
					...request,
					id: `request:${request.id}`,
					pending: true
				}))
			: []
	);
	const selectedDateRequests = $derived(
		showPendingRequests && canViewPendingRequests
			? pendingRequests.filter((request) => eventOverlapsDate(request, selectedDate))
			: []
	);
	const detailEvent = $derived(events.find((event) => event.id === detailId));
	const detailRequests = $derived(
		detailKind === 'request'
			? selectedDateRequests.filter((request) => request.id === detailId)
			: selectedDateRequests
	);
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
		[search.trim(), categoryId, tagId, audience, visibility].filter(Boolean).length
	);
	const isTodaySelected = $derived(selectedDate === todayDate);
	const categoryLabel = $derived(
		activeCategories.find((category) => category.id === categoryId)?.name ?? 'ทุกหมวดหมู่'
	);
	const tagLabel = $derived(tags.find((tag) => tag.id === tagId)?.name ?? 'ทุกแท็ก');
	const audienceLabel = $derived(
		audienceOptions.find((option) => option.value === audience)?.label
	);
	const visibilityLabel = $derived(
		visibilityOptions.find((option) => option.value === visibility)?.label
	);

	function applyEvents(result: Awaited<NonNullable<typeof data.events>>, revision: number) {
		if (!eventsRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			events = result.data;
			eventsLoaded = true;
			calendarRendered = true;
		} else error = result.error;
	}
	async function loadCalendar() {
		if (!canReadCalendar) return;
		const ticket = eventsRequest.begin();
		loading = true;
		error = '';
		applyEvents(
			await captureRouteLoad(
				listCalendarEvents(committed.filters, { signal: ticket.signal }),
				'โหลดกิจกรรมไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	async function loadPendingRequests() {
		if (!showPendingRequests || !canViewPendingRequests) return;
		const ticket = pendingRequest.begin();
		pendingLoading = true;
		pendingError = '';
		const result = await captureRouteLoad(
			listPendingCalendarRequests(calendarGridRange(selectedMonth), { signal: ticket.signal }),
			'โหลดคำร้องรออนุมัติไม่สำเร็จ'
		);
		if (
			!pendingRequest.isCurrent(ticket.revision) ||
			!showPendingRequests ||
			!canViewPendingRequests
		)
			return;
		pendingLoading = false;
		if (result.ok) {
			pendingRequests = result.data.records;
			pendingHasMore = result.data.hasMore;
			pendingLoaded = true;
		} else pendingError = result.error;
	}
	function refreshCalendar() {
		void loadCalendar();
		if (showPendingRequests) void loadPendingRequests();
	}
	function applyCategories(result: Awaited<typeof data.categories>, revision: number) {
		if (!categoriesRequest.isCurrent(revision)) return;
		categoriesLoading = false;
		if (result.ok) {
			categories = result.data;
			categoriesLoaded = true;
		} else categoriesError = result.error;
	}
	async function loadCategories() {
		if (!canReadCalendar) return;
		const ticket = categoriesRequest.begin();
		categoriesLoading = true;
		categoriesError = '';
		applyCategories(
			await captureRouteLoad(
				listCalendarCategories({ signal: ticket.signal }),
				'โหลดหมวดหมู่ไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	function applyTags(result: Awaited<typeof data.tags>, revision: number) {
		if (!tagsRequest.isCurrent(revision)) return;
		tagsLoading = false;
		if (result.ok) {
			tags = result.data;
			tagsLoaded = true;
		} else tagsError = result.error;
	}
	async function loadTags() {
		if (!canReadCalendar) return;
		const ticket = tagsRequest.begin();
		tagsLoading = true;
		tagsError = '';
		applyTags(
			await captureRouteLoad(listCalendarTags({ signal: ticket.signal }), 'โหลดแท็กไม่สำเร็จ'),
			ticket.revision
		);
	}
	function commitFilters(month = selectedMonth) {
		const url = new URL(currentUrl);
		url.searchParams.delete('academicYearId');
		url.searchParams.delete('academicTermId');
		for (const [key, value] of Object.entries({
			month: month.slice(0, 7),
			q: search.trim(),
			categoryId,
			tagId,
			audience,
			visibility
		})) {
			if (value) url.searchParams.set(key, value);
			else url.searchParams.delete(key);
		}

		goto(resolve(`staff/calendar?${url.searchParams.toString()}${url.hash}`), {
			shallow: true,
			state: { ...page.state, calendarUrl: url.href }
		});
	}
	function selectSearchEvent(event: { startDate: string }) {
		search = '';
		selectedDate = event.startDate;
		commitFilters(monthRange(event.startDate).from);
	}
	$effect.pre(() => {
		const identity = `${authStore.sessionEpoch}|${$authStore.user?.id ?? ''}|${canReadCalendar}|${canManageCalendar}|${canRequestCalendar}`;
		untrack(() => {
			if (identityOwner === identity) return;
			identityOwner = identity;
			ownerEpoch++;
			eventsRequest.abort();
			categoriesRequest.abort();
			tagsRequest.abort();
			optionsRequest.abort();
			events = [];
			eventsLoaded = false;
			calendarRendered = false;
			categories = [];
			categoriesLoaded = false;
			tags = [];
			tagsLoaded = false;
			eventDialogOpen = false;
			requestDialogOpen = false;
			requesting = false;
			showPendingRequests = false;
			filterOpen = false;
			searchOpen = false;
			detailOpen = false;
			detailAnchor = null;
			reviewTarget = null;
			reviewOpen = false;
			categoryDialogOpen = false;
			embedDialogOpen = false;
			deleteDialogOpen = false;
			saving = false;
			deleting = false;
		});
	});
	$effect.pre(() => {
		const enabled = showPendingRequests && canViewPendingRequests;
		const month = selectedMonth;
		const identity = `${authStore.sessionEpoch}|${$authStore.user?.id ?? ''}|${canManageCalendar}|${canRequestCalendar}`;
		const owner = enabled ? `${identity}|${month}` : '';
		untrack(() => {
			if (pendingOwner === owner) return;
			pendingOwner = owner;
			if (detailKind === 'request') detailOpen = false;
			pendingRequest.abort();
			pendingRequests = [];
			pendingLoading = false;
			pendingLoaded = false;
			pendingHasMore = false;
			pendingError = '';
			if (enabled) void loadPendingRequests();
		});
	});
	$effect.pre(() => {
		const operation = eventsSource,
			key = committed.key,
			allowed = canReadCalendar;
		untrack(() => {
			const changed = activeEventOwner !== key;
			if (changed) {
				activeEventOwner = key;
				detailOpen = false;
				detailAnchor = null;
				ownerEpoch++;
				events = [];
				eventsLoaded = false;
				search = committed.q;
				categoryId = committed.categoryId;
				tagId = committed.tagId;
				audience = committed.audience;
				visibility = committed.visibility;
				if (!selectedDate.startsWith(selectedMonth.slice(0, 7))) {
					selectedDate = todayDate.startsWith(selectedMonth.slice(0, 7))
						? todayDate
						: selectedMonth;
				}
				optionsRequest.abort();
				manageOptionsLoaded = false;
				manageOptionsLoading = false;
				manageOptionsError = '';
				gradeLevels = [];
				homerooms = [];
				eventDialogOpen = false;
				categoryDialogOpen = false;
				deleteDialogOpen = false;
				editingEvent = null;
				deletingEvent = null;
				saving = false;
				deleting = false;
			}
			if (!allowed) {
				eventsRequest.abort();
				loading = false;
				events = [];
				eventsLoaded = false;
				return;
			}
			if (operation && key === data.eventKey && operation !== consumedEventsSource) {
				consumedEventsSource = operation;
				const ticket = eventsRequest.begin();
				loading = true;
				error = '';
				void operation.then((result) => applyEvents(result, ticket.revision));
			} else if (changed || (operation && operation !== consumedEventsSource)) {
				consumedEventsSource = operation;
				void loadCalendar();
			}
		});
		return () => eventsRequest.abort();
	});
	$effect.pre(() => {
		const source = categoriesSource;
		untrack(() => {
			const ticket = categoriesRequest.begin();
			categoriesLoading = true;
			categoriesError = '';
			void source.then((result) => applyCategories(result, ticket.revision));
		});
		return () => categoriesRequest.abort();
	});
	$effect.pre(() => {
		const source = tagsSource;
		untrack(() => {
			const ticket = tagsRequest.begin();
			tagsLoading = true;
			tagsError = '';
			void source.then((result) => applyTags(result, ticket.revision));
		});
		return () => tagsRequest.abort();
	});
	$effect.pre(() => {
		const open = eventDialogOpen;
		untrack(() => {
			eventDraft++;
			void open;
		});
	});
	$effect.pre(() => {
		const open = categoryDialogOpen;
		untrack(() => {
			categoryDraft++;
			void open;
		});
	});
	$effect.pre(() => {
		const opened = eventDialogOpen,
			allowed = canManageCalendar;
		untrack(() => {
			if (!opened || !allowed) {
				optionsRequest.abort();
				manageOptionsLoaded = false;
				gradeLevels = [];
				homerooms = [];
				manageOptionsError = '';
				manageOptionsLoading = false;
				return;
			}
			void ensureManageOptions();
		});
		return () => optionsRequest.abort();
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		eventsRequest.abort();
		categoriesRequest.abort();
		tagsRequest.abort();
		optionsRequest.abort();
		pendingRequest.abort();
	});

	$effect(() => {
		if (!detailOpen) return;
		if (detailKind === 'event' && eventsLoaded && !detailEvent) detailOpen = false;
		if (detailKind === 'request' && pendingLoaded && !pendingLoading && !detailRequests.length)
			detailOpen = false;
	});

	function captureCalendarClick(event: MouseEvent) {
		if (!detailOpen && !dismissDetailClick) return;
		dismissDetailClick = false;
		detailOpen = false;
		event.preventDefault();
		event.stopPropagation();
	}
	function openDayDetails(date: string, anchor: HTMLElement) {
		selectedDate = date;
		detailKind = 'day';
		detailId = '';
		detailAnchor = anchor;
		detailOpen = true;
	}
	function openEntryDetails(event: CalendarDisplayEvent, date: string, anchor: HTMLElement) {
		selectedDate = date;
		detailKind = event.pending ? 'request' : 'event';
		detailId = event.pending ? event.id.slice('request:'.length) : event.id;
		detailAnchor = anchor;
		detailOpen = true;
	}
	function restoreDetailFocus(event: Event) {
		event.preventDefault();
		if (eventDialogOpen || requestDialogOpen || reviewOpen || deleteDialogOpen) return;
		if (detailAnchor?.isConnected) detailAnchor.focus();
		else
			document.querySelector<HTMLButtonElement>(`[data-calendar-date="${selectedDate}"]`)?.focus();
	}
	function reviewPendingRequest(request: PendingCalendarRequest, mode: 'approve' | 'reject') {
		if (!canReadCalendar || !canManageCalendar) return;
		detailOpen = false;
		reviewIdentity = identityOwner;
		reviewTarget = request;
		reviewMode = mode;
		reviewSession++;
		reviewOpen = true;
	}
	function patchRequestDecision(
		request: CalendarEventRequest,
		event: CalendarEvent | undefined,
		identity: string
	) {
		if (disposed || identity !== identityOwner || !canReadCalendar || !canManageCalendar) return;
		pendingRequest.abort();
		pendingLoading = false;
		pendingRequests = pendingRequests.filter((item) => item.id !== request.id);
		if (event) {
			eventsRequest.abort();
			loading = false;
			patchSavedEvent(event);
			if (!eventsLoaded) void loadCalendar();
		}
		if (showPendingRequests && (!pendingLoaded || pendingHasMore)) void loadPendingRequests();
	}
	function sortCalendarEvents(items: CalendarEvent[]) {
		return [...items].sort(
			(left, right) =>
				left.startDate.localeCompare(right.startDate) ||
				(left.startTime ?? '').localeCompare(right.startTime ?? '') ||
				left.title.localeCompare(right.title)
		);
	}

	function eventMatchesCurrentFilters(event: CalendarEvent) {
		const range = calendarGridRange(selectedMonth);
		if (event.startDate > range.to || event.endDate < range.from) return false;
		if (committed.categoryId && event.categoryId !== committed.categoryId) return false;
		if (committed.tagId && !event.tags.some((tag) => tag.id === committed.tagId)) return false;
		if (committed.visibility === 'public' && !event.isPublic) return false;
		if (committed.visibility === 'private' && event.isPublic) return false;
		if (
			committed.audience &&
			!event.targets.some((target) => target.audienceType === committed.audience)
		) {
			return false;
		}

		const query = committed.q.toLowerCase();
		if (query) {
			const searchableText = [
				event.title,
				event.description ?? '',
				event.location ?? '',
				...event.tags.map((tag) => tag.name)
			]
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
		if (
			!canManageCalendar ||
			!eventDialogOpen ||
			saving ||
			!manageOptionsLoaded ||
			manageOptionsLoading ||
			manageOptionsError
		)
			return;
		const epoch = ownerEpoch,
			draft = eventDraft,
			target = editingEvent?.id;
		const current = () => !disposed && epoch === ownerEpoch && canManageCalendar;
		const ownsDraft = () => current() && draft === eventDraft && eventDialogOpen;
		eventsRequest.abort();
		loading = false;
		const action = ++actionSequence;
		saving = true;
		try {
			const savedEvent = target
				? await updateCalendarEvent(target, payload)
				: await createCalendarEvent(payload);
			if (!current()) return;
			eventsRequest.abort();
			loading = false;
			if (eventsLoaded) patchSavedEvent(savedEvent);
			else await loadCalendar();
			if (!ownsDraft()) return;
			eventDialogOpen = false;
			editingEvent = null;
			toast.success('บันทึกกิจกรรมแล้ว');
		} catch (saveError: unknown) {
			if (!ownsDraft()) return;
			toast.error(
				(saveError instanceof Error ? saveError.message : String(saveError)) || 'บันทึกไม่สำเร็จ'
			);
		} finally {
			if (current() && action === actionSequence) saving = false;
		}
	}

	function requestDeleteEvent(event: { id: string }) {
		const target = events.find((item) => item.id === event.id);
		if (!target || !canManageCalendar) return;
		detailOpen = false;
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
		if (!target || !canManageCalendar || deleting) return;
		const epoch = ownerEpoch;
		const current = () => !disposed && epoch === ownerEpoch && canManageCalendar;
		eventsRequest.abort();
		loading = false;
		deleting = true;
		try {
			await deleteCalendarEvent(target.id);
			if (!current()) return;
			eventsRequest.abort();
			loading = false;
			events = events.filter((item) => item.id !== target.id);
			deleteDialogOpen = false;
			deletingEvent = null;
			toast.success('ลบกิจกรรมแล้ว');
		} catch (deleteError: unknown) {
			if (!current()) return;
			deleteDialogOpen = true;
			toast.error(
				(deleteError instanceof Error ? deleteError.message : String(deleteError)) ||
					'ลบกิจกรรมไม่สำเร็จ'
			);
		} finally {
			if (current()) deleting = false;
		}
	}

	async function saveCategory(
		id: string | null,
		payload: UpsertCalendarCategoryRequest
	): Promise<boolean> {
		if (!categoryDialogOpen || !canManageCalendar || saving) return false;
		const identity = identityOwner,
			draft = categoryDraft;
		const current = () => !disposed && identity === identityOwner && canManageCalendar;
		const ownsDraft = () => current() && draft === categoryDraft && categoryDialogOpen;
		categoriesRequest.abort();
		categoriesLoading = false;

		const action = ++actionSequence;
		saving = true;
		try {
			const savedCategory = id
				? await updateCalendarCategory(id, payload)
				: await createCalendarCategory(payload);
			if (!current()) return false;
			categoriesRequest.abort();
			categoriesLoading = false;
			eventsRequest.abort();
			loading = false;

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
			if (!eventsLoaded) await loadCalendar();
			if (ownsDraft()) toast.success('บันทึกหมวดหมู่แล้ว');
			return ownsDraft();
		} catch (saveError: unknown) {
			if (!ownsDraft()) return false;
			toast.error(
				(saveError instanceof Error ? saveError.message : String(saveError)) ||
					'บันทึกหมวดหมู่ไม่สำเร็จ'
			);
			return false;
		} finally {
			if (current() && action === actionSequence) saving = false;
		}
	}

	async function deleteCategory(category: CalendarCategory): Promise<boolean> {
		if (!canManageCalendar) return false;

		if (!categoryDialogOpen || !canManageCalendar || saving) return false;
		const identity = identityOwner,
			draft = categoryDraft;
		const current = () => !disposed && identity === identityOwner && canManageCalendar;
		const ownsDraft = () => current() && draft === categoryDraft && categoryDialogOpen;
		categoriesRequest.abort();
		categoriesLoading = false;

		const action = ++actionSequence;
		saving = true;
		try {
			await deleteCalendarCategory(category.id);
			if (!current()) return false;
			categoriesRequest.abort();
			categoriesLoading = false;
			eventsRequest.abort();
			loading = false;

			categories = categories.filter((item) => item.id !== category.id);
			events = events.map((event) =>
				event.categoryId === category.id
					? { ...event, categoryId: null, categoryName: null, categoryColor: null }
					: event
			);
			if (committed.categoryId === category.id) {
				const url = new URL(currentUrl);
				url.searchParams.delete('categoryId');

				goto(resolve(`staff/calendar?${url.searchParams.toString()}${url.hash}`), {
					shallow: true,
					state: { ...page.state, calendarUrl: url.href }
				});
			}
			if (!eventsLoaded) await loadCalendar();
			if (ownsDraft()) toast.success('ลบหมวดหมู่แล้ว กิจกรรมเดิมยังอยู่ครบ');
			return ownsDraft();
		} catch (deleteError: unknown) {
			if (!ownsDraft()) return false;
			toast.error(
				(deleteError instanceof Error ? deleteError.message : String(deleteError)) ||
					'ลบหมวดหมู่ไม่สำเร็จ'
			);
			return false;
		} finally {
			if (current() && action === actionSequence) saving = false;
		}
	}

	async function saveTag(id: string | null, payload: UpsertCalendarTagRequest): Promise<boolean> {
		if (!categoryDialogOpen || !canManageCalendar || saving) return false;
		const identity = identityOwner,
			draft = categoryDraft;
		const current = () => !disposed && identity === identityOwner && canManageCalendar;
		const ownsDraft = () => current() && draft === categoryDraft && categoryDialogOpen;
		tagsRequest.abort();
		tagsLoading = false;

		const action = ++actionSequence;
		saving = true;
		try {
			const savedTag = id ? await updateCalendarTag(id, payload) : await createCalendarTag(payload);
			if (!current()) return false;
			tagsRequest.abort();
			tagsLoading = false;
			eventsRequest.abort();
			loading = false;

			tags = tags.some((tag) => tag.id === savedTag.id)
				? tags.map((tag) => (tag.id === savedTag.id ? savedTag : tag))
				: [...tags, savedTag].sort((left, right) => left.name.localeCompare(right.name, 'th'));
			events = events
				.map((event) => ({
					...event,
					tags: event.tags.map((tag) =>
						tag.id === savedTag.id ? { id: savedTag.id, name: savedTag.name } : tag
					)
				}))
				.filter(eventMatchesCurrentFilters);
			if (!eventsLoaded) await loadCalendar();
			if (ownsDraft()) toast.success('บันทึกแท็กแล้ว');
			return ownsDraft();
		} catch (saveError: unknown) {
			if (!ownsDraft()) return false;
			toast.error(
				(saveError instanceof Error ? saveError.message : String(saveError)) ||
					'บันทึกแท็กไม่สำเร็จ'
			);
			return false;
		} finally {
			if (current() && action === actionSequence) saving = false;
		}
	}

	async function deleteTag(tag: CalendarTag): Promise<boolean> {
		if (!canManageCalendar) return false;

		if (!categoryDialogOpen || !canManageCalendar || saving) return false;
		const identity = identityOwner,
			draft = categoryDraft;
		const current = () => !disposed && identity === identityOwner && canManageCalendar;
		const ownsDraft = () => current() && draft === categoryDraft && categoryDialogOpen;
		tagsRequest.abort();
		tagsLoading = false;

		const action = ++actionSequence;
		saving = true;
		try {
			await deleteCalendarTag(tag.id);
			if (!current()) return false;
			tagsRequest.abort();
			tagsLoading = false;
			eventsRequest.abort();
			loading = false;

			tags = tags.filter((item) => item.id !== tag.id);
			events = events
				.map((event) => ({
					...event,
					tags: event.tags.filter((item) => item.id !== tag.id)
				}))
				.filter(eventMatchesCurrentFilters);
			if (committed.tagId === tag.id) {
				const url = new URL(currentUrl);
				url.searchParams.delete('tagId');

				goto(resolve(`staff/calendar?${url.searchParams.toString()}${url.hash}`), {
					shallow: true,
					state: { ...page.state, calendarUrl: url.href }
				});
			}
			if (!eventsLoaded) await loadCalendar();
			if (ownsDraft()) toast.success('ลบแท็กแล้ว กิจกรรมเดิมยังอยู่ครบ');
			return ownsDraft();
		} catch (deleteError: unknown) {
			if (!ownsDraft()) return false;
			toast.error(
				(deleteError instanceof Error ? deleteError.message : String(deleteError)) ||
					'ลบแท็กไม่สำเร็จ'
			);
			return false;
		} finally {
			if (current() && action === actionSequence) saving = false;
		}
	}

	async function ensureManageOptions(): Promise<boolean> {
		if (!eventDialogOpen || !canManageCalendar || !optionsDate) return false;
		if (manageOptionsLoaded) return true;
		const ticket = optionsRequest.begin();
		manageOptionsLoading = true;
		manageOptionsError = '';
		const result = await captureRouteLoad(
			listCalendarTargetOptions(optionsDate, { signal: ticket.signal }),
			'โหลดตัวเลือกชั้นเรียนไม่สำเร็จ'
		);
		if (!optionsRequest.isCurrent(ticket.revision)) return false;
		manageOptionsLoading = false;
		if (!result.ok) {
			manageOptionsError = result.error;
			return false;
		}
		({ gradeLevels, homerooms } = result.data);
		manageOptionsLoaded = true;
		return true;
	}
	function openRequestDialog() {
		if (!canRequestCalendar) return;
		detailOpen = false;
		requestSession++;
		requestError = '';
		requestDialogOpen = true;
	}
	function openEventDialog(event: { id: string } | null = null) {
		if (!canManageCalendar || !categoriesLoaded || !tagsLoaded || categoriesError || tagsError)
			return;

		detailOpen = false;
		editingEvent = event ? (events.find((item) => item.id === event.id) ?? null) : null;

		optionsDate = editingEvent?.startDate ?? selectedDate;
		eventDialogSession++;
		eventDialogOpen = true;
	}

	function changeOptionsDate(date: string) {
		if (date === optionsDate) return;
		optionsDate = date;
		manageOptionsLoaded = false;
		void ensureManageOptions();
	}
	async function submitRequest(payload: CreateCalendarRequest) {
		if (!canRequestCalendar || requesting || !requestDialogOpen) return;
		const identity = identityOwner,
			session = requestSession;
		requesting = true;
		requestError = '';
		const current = () =>
			!disposed &&
			identity === identityOwner &&
			canRequestCalendar &&
			session === requestSession &&
			requestDialogOpen;
		try {
			const request = await createCalendarRequest(payload);
			if (!current()) return;
			if (showPendingRequests) {
				if (pendingLoaded && !pendingHasMore) {
					const range = calendarGridRange(selectedMonth);
					if (request.startDate <= range.to && request.endDate >= range.from) {
						pendingRequest.abort();
						pendingLoading = false;
						const { id, title, startDate, endDate, allDay, startTime, endTime } = request;
						pendingRequests = [
							...pendingRequests.filter((item) => item.id !== id),
							{ id, title, startDate, endDate, allDay, startTime, endTime }
						];
						pendingHasMore = pendingRequests.length > 500;
						pendingRequests = pendingRequests.slice(0, 500);
					}
				} else void loadPendingRequests();
			}
			requestDialogOpen = false;
			toast.success('ส่งคำร้องแล้ว รอผู้ดูแลอนุมัติ');
		} catch (error: unknown) {
			if (current()) requestError = error instanceof Error ? error.message : 'ส่งคำร้องไม่สำเร็จ';
		} finally {
			if (!disposed && identity === identityOwner && session === requestSession) requesting = false;
		}
	}
	function openCategoryDialog() {
		if (!canManageCalendar || !categoriesLoaded || !tagsLoaded || categoriesError || tagsError)
			return;
		categoryDialogOpen = true;
	}

	async function changeMonth(offset: number) {
		const currentMonthStart = monthRange(selectedMonth).from;
		const nextMonth = monthRange(
			toIsoDate(addMonths(new Date(`${currentMonthStart}T00:00:00`), offset))
		).from;
		commitFilters(nextMonth);
	}

	async function goToToday() {
		commitFilters(todayDate);
	}

	async function resetFilters() {
		search = '';
		categoryId = '';
		tagId = '';
		audience = '';
		visibility = '';
		commitFilters();
	}

	async function copyPublicCalendarLink() {
		try {
			await navigator.clipboard.writeText(`${page.url.origin}/calendar`);
			toast.success('คัดลอกลิงก์ปฏิทินสาธารณะแล้ว');
		} catch {
			toast.error('คัดลอกลิงก์ไม่สำเร็จ');
		}
	}
</script>

<PageShell title="ปฏิทินโรงเรียน" description="กิจกรรมและประกาศตามช่วงเดือน">
	{#snippet actions()}
		<div class="flex flex-wrap gap-2">
			{#if canRequestCalendar || canManageCalendar}
				<Button
					variant="outline"
					href={resolve('staff/calendar/requests') + (canManageCalendar ? '?review=true' : '')}
				>
					<ClipboardList class="size-4" />{canManageCalendar
						? 'ติดตามและอนุมัติคำร้อง'
						: 'คำร้องของฉัน'}
				</Button>
			{/if}
			{#if canReadCalendar}
				<DropdownMenu.Root>
					<DropdownMenu.Trigger>
						{#snippet child({ props })}
							<Button {...props} variant="outline"><Settings class="size-4" />ตั้งค่า</Button>
						{/snippet}
					</DropdownMenu.Trigger>
					<DropdownMenu.Content align="end" class="w-60">
						<DropdownMenu.Item onSelect={copyPublicCalendarLink}
							><Copy class="size-4" />คัดลอกลิงก์สาธารณะ</DropdownMenu.Item
						>
						<DropdownMenu.Item onSelect={() => (embedDialogOpen = true)}
							><Code2 class="size-4" />ฝังในเว็บไซต์</DropdownMenu.Item
						>
						{#if canManageCalendar}
							<DropdownMenu.Separator />
							<DropdownMenu.Item
								onSelect={openCategoryDialog}
								disabled={!categoriesLoaded || !tagsLoaded || !!categoriesError || !!tagsError}
								><FolderPlus class="size-4" />หมวดหมู่และแท็ก</DropdownMenu.Item
							>
						{/if}
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			{/if}
			<CalendarCreateMenu
				canCreate={canManageCalendar}
				canRequest={canRequestCalendar}
				createDisabled={manageOptionsLoading ||
					!categoriesLoaded ||
					!tagsLoaded ||
					!!categoriesError ||
					!!tagsError}
				oncreate={() => openEventDialog()}
				onrequest={openRequestDialog}
			/>
		</div>
	{/snippet}

	{#if canReadCalendar || loading}
		<div class="overflow-hidden rounded-xl border bg-card">
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
					<Button
						variant="outline"
						size="icon"
						onclick={() => changeMonth(1)}
						aria-label="เดือนถัดไป"
					>
						<ChevronRight class="size-4" />
					</Button>
					<div class="min-w-0 flex-1 px-2 sm:flex-none">
						<div class="flex items-center gap-2">
							<CalendarDays class="size-4 shrink-0 text-primary" />
							<h2 class="truncate text-base font-semibold capitalize">{monthLabel}</h2>
						</div>
						<p
							role={loading && calendarRendered ? 'status' : undefined}
							class="mt-0.5 text-xs text-muted-foreground"
						>
							{#if loading && calendarRendered}กำลังโหลดกิจกรรม...
							{:else}{eventsLoaded ? selectedMonthEvents.length : '—'} กิจกรรม · {eventsLoaded
									? publicEventCount
									: '—'} สาธารณะ{/if}
						</p>
					</div>
				</div>

				<div class="flex flex-wrap items-center gap-2">
					{#if canViewPendingRequests}
						<Button
							variant={showPendingRequests ? 'secondary' : 'outline'}
							size="sm"
							aria-pressed={showPendingRequests}
							onclick={() => (showPendingRequests = !showPendingRequests)}
						>
							<ClipboardList class="size-4" />แสดงคำร้องรออนุมัติ
						</Button>
					{/if}
					<Button
						variant="outline"
						size="sm"
						aria-label="ค้นหากิจกรรม"
						onclick={() => (searchOpen = true)}><Search class="size-4" />ค้นหา</Button
					>
					<Popover.Root bind:open={filterOpen}>
						<Popover.Trigger>
							{#snippet child({ props })}
								<Button
									{...props}
									variant={activeFilterCount > 0 ? 'secondary' : 'outline'}
									size="sm"
									aria-label="เปิดตัวกรองปฏิทิน"
								>
									<SlidersHorizontal class="size-4" />ตัวกรอง
									{#if activeFilterCount > 0}<Badge variant="secondary">{activeFilterCount}</Badge
										>{/if}
								</Button>
							{/snippet}
						</Popover.Trigger>
						<Popover.Content
							align="end"
							collisionPadding={12}
							class="w-80 max-w-[calc(100vw-2rem)] max-h-96 overflow-y-auto p-4 sm:w-xl"
						>
							<form
								class="grid gap-3 sm:grid-cols-2"
								aria-label="ตัวกรองปฏิทิน"
								onsubmit={(submitEvent) => {
									submitEvent.preventDefault();
									filterOpen = false;
									detailOpen = false;
									detailAnchor = null;
									if (search.trim()) searchOpen = true;
									else commitFilters();
								}}
							>
								<div class="relative sm:col-span-2">
									<Search
										class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
									/>
									<Input
										class="pl-9"
										placeholder="ค้นหาชื่อ รายละเอียด สถานที่ หรือแท็ก"
										bind:value={search}
									/>
								</div>
								<div data-testid="calendar-categories" aria-busy={categoriesLoading}>
									{#if categoriesError}<PageState
											variant="error"
											title="โหลดหมวดหมู่ไม่สำเร็จ"
											description={categoriesError}
											actionLabel="ลองอีกครั้ง"
											onaction={loadCategories}
										/>{/if}
									{#if categoriesLoading && !categoriesLoaded}<div
											role="status"
											aria-label="กำลังโหลดหมวดหมู่"
										>
											<Skeleton class="h-9 w-full" />
										</div>{:else}<Select.Root
											type="single"
											disabled={!categoriesLoaded}
											bind:value={categoryId}
										>
											<Select.Trigger class="w-full">{categoryLabel}</Select.Trigger>
											<Select.Content>
												<Select.Item value="">ทุกหมวดหมู่</Select.Item>
												{#each activeCategories as category (category.id)}
													<Select.Item value={category.id}>{category.name}</Select.Item>
												{/each}
											</Select.Content>
										</Select.Root>{/if}
								</div>
								<div data-testid="calendar-tags" aria-busy={tagsLoading}>
									{#if tagsError}<PageState
											variant="error"
											title="โหลดแท็กไม่สำเร็จ"
											description={tagsError}
											actionLabel="ลองอีกครั้ง"
											onaction={loadTags}
										/>{/if}
									{#if tagsLoading && !tagsLoaded}<div role="status" aria-label="กำลังโหลดแท็ก">
											<Skeleton class="h-9 w-full" />
										</div>{:else}<Select.Root
											type="single"
											disabled={!tagsLoaded}
											bind:value={tagId}
										>
											<Select.Trigger class="w-full">{tagLabel}</Select.Trigger>
											<Select.Content>
												<Select.Item value="">ทุกแท็ก</Select.Item>
												{#each tags as tag (tag.id)}
													<Select.Item value={tag.id}>{tag.name}</Select.Item>
												{/each}
											</Select.Content>
										</Select.Root>{/if}
								</div>
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
								<div class="flex items-center gap-2 sm:col-span-2">
									<Button type="submit" class="flex-1">
										<SlidersHorizontal class="size-4" />
										กรอง
										{#if activeFilterCount > 0}
											<Badge variant="secondary" class="ml-1 min-w-5 justify-center px-1">
												{activeFilterCount}
											</Badge>
										{/if}
									</Button>
									{#if activeFilterCount > 0}
										<Button type="button" variant="ghost" onclick={resetFilters}>ล้างตัวกรอง</Button
										>
									{/if}
								</div>
							</form>
						</Popover.Content>
					</Popover.Root>
					<Button variant="outline" size="sm" onclick={goToToday} disabled={isTodaySelected}>
						วันนี้
					</Button>
					<Button
						variant="ghost"
						size="icon"
						onclick={refreshCalendar}
						disabled={loading}
						aria-label="รีเฟรชปฏิทิน"
					>
						<RefreshCw class={loading ? 'size-4 animate-spin' : 'size-4'} />
					</Button>
				</div>
			</div>
		</div>
	{/if}

	<section data-testid="calendar-events" aria-busy={loading}>
		{#if !canReadCalendar && !loading}
			<PageState
				variant="permission"
				title="ไม่มีสิทธิ์ดูปฏิทินโรงเรียน"
				description="ติดต่อผู้ดูแลระบบหากต้องการเข้าถึงข้อมูลนี้"
			/>
		{:else}
			{#if error}
				<PageState
					variant="error"
					title="โหลดปฏิทินไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={loadCalendar}
				/>
			{/if}
			{#if loading && !calendarRendered}<div role="status" aria-label="กำลังโหลดกิจกรรม">
					<PageSkeleton variant="detail" />
				</div>{:else if eventsLoaded || calendarRendered}
				<div class="min-w-0 space-y-3">
					{#if showPendingRequests && canViewPendingRequests && (!detailOpen || detailKind === 'event')}
						{#if pendingLoading}<p role="status" class="text-sm text-muted-foreground">
								กำลังโหลดคำร้องรออนุมัติ...
							</p>{/if}
						{#if pendingError}<PageState
								variant="error"
								title="โหลดคำร้องรออนุมัติไม่สำเร็จ"
								description={pendingError}
								actionLabel="ลองอีกครั้ง"
								onaction={loadPendingRequests}
							/>{/if}
						{#if pendingHasMore}<p role="status" class="text-sm text-muted-foreground">
								มีคำร้องมากกว่า 500 รายการ แสดงบนปฏิทินเพียง 500 รายการแรก
								กรุณาเปิดคิวคำร้องเพื่อดูทั้งหมด
							</p>{/if}
					{/if}
					<div
						role="presentation"
						onpointerdowncapture={() => (dismissDetailClick = detailOpen)}
						onclickcapture={captureCalendarClick}
					>
						<CalendarMonthGrid
							monthDate={selectedMonth}
							{events}
							pendingRequests={pendingDisplayEvents}
							{selectedDate}
							hideMobileTimes
							onselect={openDayDetails}
							oneventselect={openEntryDetails}
						/>
					</div>

					{#if activeCategories.length > 0}<CalendarColorKey items={activeCategories} />{/if}
				</div>
				<Popover.Root bind:open={detailOpen}>
					<Popover.Content
						customAnchor={detailAnchor}
						side="bottom"
						align="start"
						collisionPadding={12}
						sideOffset={8}
						onCloseAutoFocus={restoreDetailFocus}
						role={detailOpen ? 'dialog' : undefined}
						aria-hidden={!detailOpen}
						aria-label="รายละเอียดปฏิทิน"
						class="max-h-[min(80dvh,var(--bits-popover-content-available-height))] w-md max-w-[calc(100vw-2rem)] space-y-4 overflow-y-auto"
					>
						<div class="flex items-center justify-between gap-3">
							<h2 class="font-semibold">
								{detailKind === 'day' ? formatCalendarDate(selectedDate) : 'รายละเอียดกิจกรรม'}
							</h2>
							<div class="flex items-center gap-1">
								{#if detailKind === 'day'}
									<CalendarCreateMenu
										canCreate={canManageCalendar}
										canRequest={canRequestCalendar}
										compact
										label="เพิ่มรายการในวันที่เลือก"
										createDisabled={manageOptionsLoading ||
											!categoriesLoaded ||
											!tagsLoaded ||
											!!categoriesError ||
											!!tagsError}
										oncreate={() => openEventDialog()}
										onrequest={openRequestDialog}
									/>
								{/if}
								<Button
									variant="ghost"
									size="icon"
									aria-label="ปิดรายละเอียดกิจกรรม"
									onclick={() => (detailOpen = false)}><X class="size-4" /></Button
								>
							</div>
						</div>
						{#if detailKind !== 'request' && !eventsLoaded && loading}
							<p role="status" class="text-sm text-muted-foreground">กำลังโหลดกิจกรรม...</p>
						{:else if detailKind !== 'request' && eventsLoaded}
							<CalendarEventList
								events={detailKind === 'event'
									? detailEvent
										? [detailEvent]
										: []
									: selectedDateEvents}
								variant="plain"
								showFullDescription
								canManage={canManageCalendar}
								onedit={openEventDialog}
								ondelete={requestDeleteEvent}
							/>
						{/if}
						{#if showPendingRequests && canViewPendingRequests && detailKind !== 'event'}
							<section
								aria-label="คำร้องในวันที่เลือก"
								aria-busy={pendingLoading}
								class="space-y-3 rounded-xl border border-dashed border-primary/50 bg-card p-4"
							>
								<div class="flex flex-wrap items-center justify-between gap-2">
									<h3 class="font-semibold">คำร้องรออนุมัติ</h3>
									<Button
										variant="outline"
										size="sm"
										href={resolve('staff/calendar/requests') +
											(canManageCalendar ? '?review=true&status=pending' : '?status=pending')}
									>
										{canManageCalendar ? 'เปิดคิวอนุมัติ' : 'คำร้องของฉัน'}
									</Button>
								</div>
								{#if pendingLoading}<p role="status" class="text-sm text-muted-foreground">
										กำลังโหลดคำร้องรออนุมัติ...
									</p>{/if}
								{#if pendingError}<PageState
										variant="error"
										title="โหลดคำร้องรออนุมัติไม่สำเร็จ"
										description={pendingError}
										actionLabel="ลองอีกครั้ง"
										onaction={loadPendingRequests}
									/>{/if}
								{#if pendingHasMore}<p role="status" class="text-sm text-muted-foreground">
										มีคำร้องมากกว่า 500 รายการ แสดงบนปฏิทินเพียง 500 รายการแรก
										กรุณาเปิดคิวคำร้องเพื่อดูทั้งหมด
									</p>{/if}
								{#each detailRequests as request (request.id)}
									<article class="space-y-1 rounded-lg border border-dashed bg-background p-3">
										<div class="flex items-start justify-between gap-2">
											<h4 class="min-w-0 break-words font-medium">{request.title}</h4>
											<Badge variant="outline" class="shrink-0">รออนุมัติ</Badge>
										</div>
										<p class="text-sm text-muted-foreground">
											{request.allDay
												? 'ทั้งวัน'
												: `${request.startTime?.slice(0, 5)} – ${request.endTime?.slice(0, 5)}`}
										</p>
										{#if request.startDate !== request.endDate}<p
												class="text-sm text-muted-foreground"
											>
												{formatCalendarDate(request.startDate)} – {formatCalendarDate(
													request.endDate
												)}
											</p>{/if}
										{#if canManageCalendar}<div class="flex flex-wrap gap-2 pt-2">
												<Button size="sm" onclick={() => reviewPendingRequest(request, 'approve')}
													>ตรวจและอนุมัติ</Button
												>
												<Button
													variant="outline"
													size="sm"
													onclick={() => reviewPendingRequest(request, 'reject')}>ไม่อนุมัติ</Button
												>
											</div>{/if}
									</article>
								{:else}
									{#if pendingLoaded && !pendingError && !pendingLoading}<p
											class="text-sm text-muted-foreground"
										>
											วันนี้ไม่มีคำร้องรออนุมัติ
										</p>{/if}
								{/each}
							</section>
						{/if}
					</Popover.Content>
				</Popover.Root>
			{/if}
		{/if}
	</section>
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

	{#if eventDialogOpen && canManageCalendar}
		{#key eventDialogSession}
			<CalendarEventDialog
				bind:open={eventDialogOpen}
				{categories}
				{tags}
				{gradeLevels}
				{homerooms}
				initialDate={selectedDate}
				ondatechange={changeOptionsDate}
				event={editingEvent}
				{saving}
				optionsLoading={manageOptionsLoading}
				optionsLoaded={manageOptionsLoaded}
				optionsError={manageOptionsError}
				onretryoptions={ensureManageOptions}
				onsave={saveEvent}
			/>
		{/key}
	{/if}
	{#if reviewTarget && canReadCalendar && canManageCalendar}
		{#key reviewSession}
			{@const decisionOwner = reviewIdentity}
			<CalendarRequestReviewDialog
				bind:open={reviewOpen}
				target={reviewTarget}
				mode={reviewMode}
				initialCatalogs={categoriesLoaded && tagsLoaded && !categoriesError && !tagsError
					? { categories, tags }
					: undefined}
				ondecided={(request, event) => patchRequestDecision(request, event, decisionOwner)}
			/>
		{/key}
	{/if}
	{#if requestDialogOpen && canRequestCalendar}
		{#key requestSession}
			<CalendarRequestDialog
				bind:open={requestDialogOpen}
				initialDate={selectedDate}
				saving={requesting}
				error={requestError}
				onsubmit={submitRequest}
			/>
		{/key}
	{/if}
	{#if categoryDialogOpen && canManageCalendar}
		<CalendarCategoryDialog
			bind:open={categoryDialogOpen}
			{categories}
			{tags}
			{saving}
			onsavecategory={saveCategory}
			ondeletecategory={deleteCategory}
			onsavetag={saveTag}
			ondeletetag={deleteTag}
		/>
	{/if}
	{#if embedDialogOpen}<CalendarEmbedDialog
			bind:open={embedDialogOpen}
			origin={page.url.origin}
		/>{/if}
</PageShell>

{#if canReadCalendar}
	<CalendarSearchDialog
		bind:open={searchOpen}
		kind="staff"
		initialQuery={search}
		filters={{
			categoryId: categoryId || undefined,
			tagId: tagId || undefined,
			audience: audience || undefined,
			visibility: visibility || undefined
		}}
		onselect={selectSearchEvent}
	/>
{/if}
