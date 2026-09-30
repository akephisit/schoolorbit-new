<script lang="ts">
	import { page } from '$app/state';
	import { addMonths } from 'date-fns';
	import { onDestroy, untrack } from 'svelte';
	import { pushState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import { calendarRouteFilters } from '$lib/utils/calendar-route-filters';
	import { authStore } from '$lib/stores/auth';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import { toast } from 'svelte-sonner';

	import {
		listGradeLevelOptions,
		listHomerooms,
		type GradeLevelOption,
		type Homeroom
	} from '$lib/api/academic-core';
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
	import CalendarEmbedDialog from '$lib/components/calendar/CalendarEmbedDialog.svelte';
	import CalendarColorKey from '$lib/components/calendar/CalendarColorKey.svelte';
	import {
		type CalendarAudienceType,
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
	} from '$lib/api/calendar';
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
		Code2,
		Copy,
		FolderPlus,
		Plus,
		RefreshCw,
		Search,
		SlidersHorizontal
	} from '@lucide/svelte';

	type VisibilityFilter = '' | 'public' | 'private';
	type AudienceFilter = '' | CalendarAudienceType;

	const todayDate = toIsoDate(new Date());
	const currentUrl = $derived(page.state.calendarUrl ? new URL(page.state.calendarUrl) : page.url);
	const committed = $derived(calendarRouteFilters(currentUrl));
	const academicYearId = $derived(committed.academicYearId);
	const academicTermId = $derived(committed.academicTermId);
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
	let search = $state('');
	let categoryId = $state('');
	let tagId = $state('');
	let audience = $state<AudienceFilter>('');
	let visibility = $state<VisibilityFilter>('');
	let eventDialogOpen = $state(false);
	let eventDialogSession = $state(0);
	let categoryDialogOpen = $state(false);
	let embedDialogOpen = $state(false);
	let editingEvent = $state<CalendarEvent | null>(null);
	let saving = $state(false);
	let error = $state('');
	let gradeLevels = $state.raw<GradeLevelOption[]>([]);
	let homerooms = $state.raw<Homeroom[]>([]);
	let manageOptionsLoaded = $state(false);
	let manageOptionsLoading = $state(false);
	let deleteDialogOpen = $state(false);
	let deletingEvent = $state<CalendarEvent | null>(null);
	let deleting = $state(false);
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
	let consumedEventsSource: typeof data.events = null;
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
		} else error = result.error;
	}
	async function loadCalendar() {
		if (!canReadCalendar || !academicYearId) return;
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
		pushState(resolve(`/staff/calendar?${url.searchParams.toString()}${url.hash}`), {
			...page.state,
			calendarUrl: url.href
		});
	}
	$effect.pre(() => {
		const identity = `${$authStore.user?.id ?? ''}|${canReadCalendar}|${canManageCalendar}`;
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
			categories = [];
			categoriesLoaded = false;
			tags = [];
			tagsLoaded = false;
			eventDialogOpen = false;
			categoryDialogOpen = false;
			embedDialogOpen = false;
			deleteDialogOpen = false;
			saving = false;
			deleting = false;
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
				ownerEpoch++;
				events = [];
				eventsLoaded = false;
				search = committed.q;
				categoryId = committed.categoryId;
				tagId = committed.tagId;
				audience = committed.audience;
				visibility = committed.visibility;
				selectedDate = todayDate.startsWith(selectedMonth.slice(0, 7)) ? todayDate : selectedMonth;
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
			if (!academicYearId) {
				eventsRequest.abort();
				loading = true;
				error = '';
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
			allowed = canManageCalendar,
			year = academicYearId;
		untrack(() => {
			if (!opened || !allowed || !year) {
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
	});

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
			if (!eventsLoaded && academicYearId) await loadCalendar();
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
				pushState(resolve(`/staff/calendar?${url.searchParams.toString()}${url.hash}`), {
					...page.state,
					calendarUrl: url.href
				});
			}
			if (!eventsLoaded && academicYearId) await loadCalendar();
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
			if (!eventsLoaded && academicYearId) await loadCalendar();
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
				pushState(resolve(`/staff/calendar?${url.searchParams.toString()}${url.hash}`), {
					...page.state,
					calendarUrl: url.href
				});
			}
			if (!eventsLoaded && academicYearId) await loadCalendar();
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
		if (!eventDialogOpen || !canManageCalendar || !academicYearId) return false;
		if (manageOptionsLoaded) return true;
		const ticket = optionsRequest.begin();
		manageOptionsLoading = true;
		manageOptionsError = '';
		const result = await captureRouteLoad(
			Promise.all([
				listGradeLevelOptions(academicYearId, { signal: ticket.signal }),
				listHomerooms(academicYearId, { signal: ticket.signal })
			]),
			'โหลดตัวเลือกชั้นเรียนไม่สำเร็จ'
		);
		if (!optionsRequest.isCurrent(ticket.revision)) return false;
		manageOptionsLoading = false;
		if (!result.ok) {
			manageOptionsError = result.error;
			return false;
		}
		[gradeLevels, homerooms] = result.data;
		manageOptionsLoaded = true;
		return true;
	}
	function openEventDialog(event: { id: string } | null = null) {
		if (!canManageCalendar || !categoriesLoaded || !tagsLoaded || categoriesError || tagsError)
			return;
		editingEvent = event ? (events.find((item) => item.id === event.id) ?? null) : null;
		eventDialogSession++;
		eventDialogOpen = true;
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
			{#if canReadCalendar}
				<Button variant="outline" onclick={copyPublicCalendarLink}>
					<Copy class="size-4" />
					คัดลอกลิงก์สาธารณะ
				</Button>
				<Button variant="outline" onclick={() => (embedDialogOpen = true)}>
					<Code2 class="size-4" />
					ฝังในเว็บไซต์
				</Button>
			{/if}
			{#if canManageCalendar}
				<Button
					variant="outline"
					onclick={openCategoryDialog}
					disabled={!categoriesLoaded || !tagsLoaded || !!categoriesError || !!tagsError}
				>
					<FolderPlus class="h-4 w-4" />
					หมวดหมู่และแท็ก
				</Button>
				<Button
					onclick={() => openEventDialog()}
					disabled={manageOptionsLoading ||
						!categoriesLoaded ||
						!tagsLoaded ||
						!!categoriesError ||
						!!tagsError}
				>
					<Plus class="h-4 w-4" />
					เพิ่มกิจกรรม
				</Button>
			{/if}
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
					<div class="min-w-0 flex-1 px-2 sm:min-w-52 sm:flex-none">
						<div class="flex items-center gap-2">
							<CalendarDays class="size-4 shrink-0 text-primary" />
							<h2 class="truncate text-base font-semibold capitalize">{monthLabel}</h2>
						</div>
						<p class="mt-0.5 text-xs text-muted-foreground">
							{eventsLoaded ? selectedMonthEvents.length : '—'} กิจกรรม · {eventsLoaded
								? publicEventCount
								: '—'} สาธารณะ
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
				class="grid gap-3 p-3 sm:grid-cols-2 sm:p-4 xl:grid-cols-[minmax(220px,1fr)_160px_160px_160px_160px_auto]"
				onsubmit={(submitEvent) => {
					submitEvent.preventDefault();
					commitFilters();
				}}
			>
				<div class="relative sm:col-span-2 xl:col-span-1">
					<Search class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
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
						</div>{:else}<Select.Root type="single" disabled={!tagsLoaded} bind:value={tagId}>
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

	<section data-testid="calendar-events" aria-busy={loading}>
		{#if !canReadCalendar && !loading}
			<PageState
				variant="permission"
				title="ไม่มีสิทธิ์ดูปฏิทินโรงเรียน"
				description="ติดต่อผู้ดูแลระบบหากต้องการเข้าถึงข้อมูลนี้"
			/>
		{:else}
			{#if loading && eventsLoaded}<p role="status" class="text-sm text-muted-foreground">
					กำลังอัปเดตกิจกรรม...
				</p>{/if}
			{#if error}
				<PageState
					variant="error"
					title="โหลดปฏิทินไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={loadCalendar}
				/>
			{/if}
			{#if loading && !eventsLoaded}<div role="status" aria-label="กำลังโหลดกิจกรรม">
					<PageSkeleton variant="detail" />
				</div>{:else if eventsLoaded}
				<div class="grid items-start gap-5 xl:grid-cols-[minmax(0,1fr)_400px]">
					<div class="min-w-0 space-y-3">
						<CalendarMonthGrid
							monthDate={selectedMonth}
							{events}
							{selectedDate}
							onselect={(date) => (selectedDate = date)}
						/>
						{#if activeCategories.length > 0}
							<CalendarColorKey items={activeCategories} />
						{/if}
					</div>
					<section class="space-y-3">
						<div class="flex items-center gap-3 rounded-xl border bg-card p-4">
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
				academicYearId={academicYearId ?? ''}
				{academicTermId}
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
