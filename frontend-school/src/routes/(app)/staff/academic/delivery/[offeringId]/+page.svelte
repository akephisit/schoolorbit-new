<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { onDestroy, onMount, tick, untrack } from 'svelte';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { get } from 'svelte/store';
	import { toast } from 'svelte-sonner';
	import {
		getLearningOffering,
		listLearningGroups,
		getLearningGroup,
		getLearningDeliveryManagementOptions,
		createLearningGroup,
		updateLearningGroup,
		replaceLearningGroupTeachers,
		replaceLearningGroupHomerooms,
		previewLearningGroupRoster,
		applyLearningGroupRoster,
		updateWeeklyPeriodTarget,
		type LearningOffering,
		type LearningGroup,
		type DeliveryManagementOptions,
		type RosterPreview,
		type CreateLearningGroupRequest,
		type UpdateLearningGroupRequest,
		type ReplaceLearningGroupTeachersRequest,
		type ReplaceLearningGroupHomeroomsRequest,
		type LearningGroupRosterTracking,
		type BulkGroupSetupResponse
	} from '#lib/api/learning-delivery.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import {
		deliveryReturnHref,
		repairedDeliveryOfferingHref
	} from '#lib/navigation/learning-delivery.js';
	import {
		connectTimetableSocket,
		disconnectTimetableSocket,
		refreshTrigger
	} from '#lib/stores/timetable-socket.js';
	import { registerDeliveryReconcile } from '#lib/academic/delivery-reconcile.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import LearningGroupList from '#lib/components/learning-delivery/LearningGroupList.svelte';
	import LearningGroupEditor from '#lib/components/learning-delivery/LearningGroupEditor.svelte';
	import RosterPreviewPanel from '#lib/components/learning-delivery/RosterPreviewPanel.svelte';
	import RosterTrackingPanel from '#lib/components/learning-delivery/RosterTrackingPanel.svelte';
	import DatedRosterMemberships from '#lib/components/learning-delivery/DatedRosterMemberships.svelte';
	import DeliveryBulkGroupSetupDialog from '#lib/components/learning-delivery/DeliveryBulkGroupSetupDialog.svelte';
	import DeliveryResourceDeletion from '#lib/components/learning-delivery/DeliveryResourceDeletion.svelte';
	let { data }: { data: PageData } = $props();
	const academicContext = getAcademicContextStore();
	let offering = $state.raw<LearningOffering | null>(null),
		groups = $state.raw<LearningGroup[]>([]),
		selectedGroup = $state.raw<LearningGroup | null>(null),
		options = $state.raw<DeliveryManagementOptions | null>(null),
		preview = $state.raw<RosterPreview | null>(null),
		tracking = $state.raw<LearningGroupRosterTracking | null>(null);
	let offeringLoading = $state(true),
		groupsLoading = $state(true),
		groupLoading = $state(false),
		optionsLoading = $state(false),
		rosterLoading = $state(false),
		weeklySaving = $state(false);
	let offeringError = $state(''),
		groupsError = $state(''),
		groupError = $state(''),
		optionsError = $state(''),
		weeklyError = $state('');
	let weeklyPeriodTarget = $state(0),
		weeklyBaseline = $state(0),
		weeklyConflict = $state(false),
		editingGroup = $state(false),
		previewStale = $state(false);
	const offeringRequest = new LatestRequest(),
		groupsRequest = new LatestRequest(),
		groupRequest = new LatestRequest(),
		optionsRequest = new LatestRequest(),
		rosterRequest = new LatestRequest();
	let editorBusy = $state(false),
		editorSession = 0;
	let editorTrigger = $state<HTMLButtonElement | null>(null);
	let pendingGroupId = '';
	const identityEpoch = $derived.by(() => {
		void $authStore;
		return authStore.sessionEpoch;
	});
	let activeIdentityEpoch = -1;
	let activeOfferingId = '',
		disposed = false,
		offeringRevision = 0,
		groupsRevision = 0,
		groupRevision = 0;
	const routeEpochs = new WeakMap<object, number>();
	let initialGroupSeed = $state.raw<{
		offeringId: string;
		groupId: string;
		epoch: number;
		tracking: PageData['rosterTracking'];
		memberships: PageData['memberships'];
	} | null>(null);
	const initialGroupSeedIsCurrent = $derived.by(() => {
		void $authStore.user;
		return (
			initialGroupSeed !== null &&
			initialGroupSeed.offeringId === offering?.id &&
			initialGroupSeed.groupId === selectedGroup?.id &&
			initialGroupSeed.epoch === authStore.sessionEpoch
		);
	});
	const ownerContextReady = $derived(
		Boolean(
			offering &&
			page.url.searchParams.get('academicYearId') === offering.academicYearId &&
			page.url.searchParams.get('academicTermId') === offering.academicTermId &&
			$academicContext.selected.academicYearId === offering.academicYearId &&
			$academicContext.selected.academicTermId === offering.academicTermId
		)
	);
	const termAllowsEditing = $derived(
		!['closed', 'cancelled'].includes(
			$academicContext.options?.terms.find((term) => term.id === offering?.academicTermId)
				?.status ?? 'planning'
		)
	);
	const canManage = $derived(
		Boolean(offering?.canManage) &&
			ownerContextReady &&
			termAllowsEditing &&
			$can.hasAny(
				PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
				PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
				PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
				PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
			)
	);
	const canDelete = $derived(
		ownerContextReady && termAllowsEditing && $can.has(PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL)
	);
	const backHref = $derived(
		offering
			? deliveryReturnHref(new URL((page.shallow?.url ?? page.url).href), offering)
			: '/staff/academic/delivery'
	);
	let contextRepairKey = '',
		contextRepairRevision = 0;
	let contextRepairError = $state('');
	async function repairOwnerContext(force = false) {
		const value = offering,
			id = data.offeringId,
			epoch = authStore.sessionEpoch,
			href = page.url.href;
		if (!value || value.id !== id || activeOfferingId !== id) return;
		const target = repairedDeliveryOfferingHref(new URL(href), value);
		if (!target) {
			contextRepairKey = '';
			contextRepairError = '';
			return;
		}
		const key = `${epoch}:${id}:${href}`;
		if (!force && contextRepairKey === key) return;
		contextRepairKey = key;
		const revision = ++contextRepairRevision;
		contextRepairError = '';
		try {
			await goto(target, { replace: true, reset: false, state: page.state });
		} catch (error) {
			if (
				!disposed &&
				data.offeringId === id &&
				activeOfferingId === id &&
				authStore.sessionEpoch === epoch &&
				page.url.href === href &&
				revision === contextRepairRevision
			)
				contextRepairError =
					error instanceof Error ? error.message : 'ปรับบริบทของรายการเปิดสอนไม่สำเร็จ';
		}
	}
	$effect(() => {
		void offering;
		void page.url.href;
		void identityEpoch;
		void data.offeringId;
		untrack(() => void repairOwnerContext());
	});
	function routeEpoch(promise: object) {
		const known = routeEpochs.get(promise);
		if (known !== undefined) return known;
		const epoch = authStore.sessionEpoch;
		routeEpochs.set(promise, epoch);
		return epoch;
	}
	function admission(groupId?: string) {
		const id = activeOfferingId,
			epoch = authStore.sessionEpoch;
		return () =>
			!disposed &&
			activeOfferingId === id &&
			ownerContextReady &&
			authStore.sessionEpoch === epoch &&
			(!groupId || selectedGroup?.id === groupId);
	}
	function acceptOffering(value: LearningOffering) {
		if (value.id !== activeOfferingId || (offering && offering.rowVersion > value.rowVersion))
			return;
		const dirty = weeklyPeriodTarget !== weeklyBaseline;
		weeklyConflict =
			dirty &&
			value.weeklyPeriodTarget !== weeklyBaseline &&
			value.weeklyPeriodTarget !== weeklyPeriodTarget;
		offering = value;
		if (!dirty || value.weeklyPeriodTarget === weeklyPeriodTarget) {
			weeklyPeriodTarget = value.weeklyPeriodTarget;
			weeklyBaseline = value.weeklyPeriodTarget;
			weeklyConflict = false;
		}
	}
	function acceptGroup(value: LearningGroup) {
		if (value.learningOfferingId !== activeOfferingId) return;
		groups = groups.some((g) => g.id === value.id)
			? groups.map((g) => (g.id === value.id && g.rowVersion <= value.rowVersion ? value : g))
			: [...groups, value];
		if (selectedGroup?.id === value.id && selectedGroup.rowVersion <= value.rowVersion)
			selectedGroup = value;
	}
	async function loadOffering() {
		const current = admission(),
			ticket = offeringRequest.begin();
		offeringRevision++;
		offeringLoading = true;
		offeringError = '';
		try {
			const loaded = await getLearningOffering(activeOfferingId, { signal: ticket.signal });
			if (current() && offeringRequest.isCurrent(ticket.revision)) acceptOffering(loaded);
		} catch (error) {
			if (current() && offeringRequest.isCurrent(ticket.revision) && !isAbortError(error)) {
				offeringError = error instanceof Error ? error.message : 'โหลดรายการเปิดสอนไม่สำเร็จ';
				if (error instanceof ApiClientError && error.status === 404) {
					offering = null;
					groups = [];
					selectedGroup = null;
					options = null;
					preview = null;
					tracking = null;
				}
			}
		} finally {
			if (current() && offeringRequest.isCurrent(ticket.revision)) offeringLoading = false;
		}
	}

	async function loadGroups(
		signal?: AbortSignal
	): Promise<RouteLoadResult<LearningGroup[]> | null> {
		const current = admission(),
			ticket = groupsRequest.begin();
		groupsRevision++;
		groupsLoading = true;
		groupsError = '';
		try {
			const loaded = await listLearningGroups(activeOfferingId, {
				signal: signal ? AbortSignal.any([signal, ticket.signal]) : ticket.signal
			});
			if (!current() || !groupsRequest.isCurrent(ticket.revision)) return null;
			groups = loaded;
			if (selectedGroup && !groups.some((g) => g.id === selectedGroup?.id)) {
				selectedGroup = null;
				editingGroup = false;
				editorSession++;
				editorBusy = false;
				preview = null;
				tracking = null;
			} else if (selectedGroup) acceptGroup(groups.find((g) => g.id === selectedGroup?.id)!);
			return { ok: true, data: loaded, error: null };
		} catch (error) {
			if (!current() || !groupsRequest.isCurrent(ticket.revision) || isAbortError(error))
				return null;
			groupsError = error instanceof Error ? error.message : 'โหลดกลุ่มไม่สำเร็จ';
			return { ok: false, data: null, error: groupsError };
		} finally {
			if (current() && groupsRequest.isCurrent(ticket.revision)) groupsLoading = false;
		}
	}
	async function selectGroupId(id: string, navigate = true) {
		const current = admission(),
			ticket = groupRequest.begin();
		groupRevision++;
		initialGroupSeed = null;
		selectedGroup = groups.find((group) => group.id === id) ?? null;
		editingGroup = false;
		editorSession++;
		editorBusy = false;
		preview = null;
		tracking = null;
		groupLoading = true;
		groupError = '';
		pendingGroupId = id;
		if (navigate) {
			const url = new URL((page.shallow?.url ?? page.url).href);
			url.searchParams.set('groupId', id);
			void goto(url.pathname + url.search, { shallow: true, state: page.state });
		}
		try {
			const loaded = await getLearningGroup(id, { signal: ticket.signal });
			if (current() && groupRequest.isCurrent(ticket.revision) && pendingGroupId === id) {
				if (loaded.learningOfferingId !== activeOfferingId)
					throw new Error('กลุ่มเรียนไม่อยู่ในรายการเปิดสอนนี้');
				selectedGroup = loaded;
				acceptGroup(loaded);
			}
		} catch (error) {
			if (current() && groupRequest.isCurrent(ticket.revision) && !isAbortError(error))
				groupError = error instanceof Error ? error.message : 'โหลดกลุ่มไม่สำเร็จ';
		} finally {
			if (current() && groupRequest.isCurrent(ticket.revision)) {
				groupLoading = false;
				pendingGroupId = '';
			}
		}
	}
	function selectGroup(group: LearningGroup) {
		return selectGroupId(group.id);
	}
	function openGroupEditor() {
		if (!canManage || editorBusy) return;
		editorSession++;
		editorBusy = false;
		editingGroup = true;
		void loadOptions();
	}
	function editorBusyObserver() {
		const token = editorSession,
			id = selectedGroup?.id,
			epoch = authStore.sessionEpoch;
		return (busy: boolean) => {
			if (
				!disposed &&
				editingGroup &&
				editorSession === token &&
				selectedGroup?.id === id &&
				authStore.sessionEpoch === epoch
			)
				editorBusy = busy;
		};
	}
	async function cancelGroupEditor() {
		if (editorBusy) return;
		const current = admission(selectedGroup?.id);
		editingGroup = false;
		editorSession++;
		await tick();
		if (current() && !editingGroup) editorTrigger?.focus();
	}

	async function loadOptions(): Promise<DeliveryManagementOptions | null> {
		if (options) return options;
		if (!offering || !canManage) return null;
		const current = admission(),
			ticket = optionsRequest.begin();
		optionsLoading = true;
		optionsError = '';
		try {
			const loaded = await getLearningDeliveryManagementOptions(offering.academicTermId, {
				signal: ticket.signal
			});
			if (current() && optionsRequest.isCurrent(ticket.revision)) {
				options = loaded;
				return loaded;
			}
			return null;
		} catch (error) {
			if (current() && optionsRequest.isCurrent(ticket.revision) && !isAbortError(error))
				optionsError = error instanceof Error ? error.message : 'โหลดตัวเลือกไม่สำเร็จ';
			return null;
		} finally {
			if (current() && optionsRequest.isCurrent(ticket.revision)) optionsLoading = false;
		}
	}
	async function createGroup(body: CreateLearningGroupRequest) {
		if (!canManage) return;
		const current = admission();
		const created = await createLearningGroup(activeOfferingId, body);
		if (current()) {
			groupsRevision++;
			acceptGroup(created);
			await selectGroup(created);
		}
	}
	async function saveGroup(body: UpdateLearningGroupRequest) {
		if (!selectedGroup || !canManage) throw new Error('กลุ่มนี้ไม่พร้อมแก้ไข');
		const id = selectedGroup.id,
			current = admission(id);
		const value = await updateLearningGroup(id, body);
		if (current()) {
			groupRevision++;
			acceptGroup(value);
		}
		return value;
	}
	async function saveTeachers(body: ReplaceLearningGroupTeachersRequest) {
		if (!selectedGroup || !canManage) throw new Error('กลุ่มนี้ไม่พร้อมแก้ไข');
		const id = selectedGroup.id,
			current = admission(id);
		const value = await replaceLearningGroupTeachers(id, body);
		if (current()) {
			groupRevision++;
			acceptGroup(value);
		}
		return value;
	}
	async function saveHomerooms(body: ReplaceLearningGroupHomeroomsRequest) {
		if (!selectedGroup || !canManage) throw new Error('กลุ่มนี้ไม่พร้อมแก้ไข');
		const id = selectedGroup.id,
			current = admission(id);
		const value = await replaceLearningGroupHomerooms(id, body);
		if (current()) {
			groupRevision++;
			acceptGroup(value);
			previewStale = true;
		}
		return value;
	}
	async function refreshRoster() {
		if (!selectedGroup) return;
		const id = selectedGroup.id,
			current = admission(id),
			ticket = rosterRequest.begin();
		rosterLoading = true;
		try {
			const result = await previewLearningGroupRoster(id);
			if (current() && rosterRequest.isCurrent(ticket.revision)) {
				preview = result;
				previewStale = false;
			}
		} finally {
			if (current() && rosterRequest.isCurrent(ticket.revision)) rosterLoading = false;
		}
	}
	async function applyRoster(sourceHash: string) {
		if (!selectedGroup || !canManage) return;
		const id = selectedGroup.id,
			current = admission(id);
		const value = await applyLearningGroupRoster(id, {
			rowVersion: selectedGroup.rowVersion,
			sourceHash,
			overrides: []
		});
		if (current()) {
			groupRevision++;
			acceptGroup(value);
			preview = null;
			toast.success('บันทึกรายชื่อแล้ว');
		}
	}
	async function refresh() {
		await Promise.all([loadOffering(), loadGroups()]);
	}
	async function saveWeekly(event: SubmitEvent) {
		event.preventDefault();
		if (
			!offering ||
			!canManage ||
			weeklySaving ||
			weeklyConflict ||
			!Number.isInteger(weeklyPeriodTarget) ||
			weeklyPeriodTarget < 0
		)
			return;
		const current = admission(),
			id = offering.id;
		weeklySaving = true;
		weeklyError = '';
		try {
			const value = await updateWeeklyPeriodTarget(id, {
				rowVersion: offering.rowVersion,
				weeklyPeriodTarget
			});
			if (current()) {
				offeringRevision++;
				acceptOffering(value);
				toast.success('บันทึกจำนวนคาบแล้ว');
			}
		} catch (error) {
			if (current())
				weeklyError = error instanceof Error ? error.message : 'บันทึกจำนวนคาบไม่สำเร็จ';
		} finally {
			if (current()) weeklySaving = false;
		}
	}
	function trackingUpdated(id: string, config: LearningGroupRosterTracking) {
		if (selectedGroup?.id !== id) return;
		tracking = config;
		acceptGroup({ ...selectedGroup, rowVersion: config.groupRowVersion });
		previewStale = true;
	}

	function bulkApplied(response: BulkGroupSetupResponse) {
		groupsRequest.abort();
		groupRequest.abort();
		groupsRevision++;
		groupRevision++;
		groupsLoading = false;
		groupLoading = false;
		pendingGroupId = '';
		for (const group of response.groups) {
			const config = response.rosterTracking.find(
				(entry) => entry.learningGroupId === group.id
			)?.tracking;
			const observed = groups.find((entry) => entry.id === group.id);
			if (
				!config ||
				config.groupRowVersion !== group.rowVersion ||
				(observed && observed.rowVersion > group.rowVersion)
			)
				continue;
			if (selectedGroup?.id === group.id && tracking && tracking.groupRowVersion > group.rowVersion)
				continue;
			if (selectedGroup?.id === group.id) tracking = config;
			acceptGroup(group);
		}
	}
	let observedGroupRoute = '',
		observedGroupOwner = '';
	$effect(() => {
		const owner = `${data.offeringId}:${identityEpoch}`,
			id = (page.shallow?.url ?? page.url).searchParams.get('groupId') ?? '';
		untrack(() => {
			if (observedGroupOwner !== owner) {
				observedGroupOwner = owner;
				observedGroupRoute = id;
				return;
			}
			if (observedGroupRoute === id) return;
			observedGroupRoute = id;
			const target = id || groups[0]?.id;
			if (!target || selectedGroup?.id === target || pendingGroupId === target) return;
			void selectGroupId(target, false);
		});
	});

	$effect.pre(() => {
		const id = data.offeringId,
			epoch = identityEpoch,
			routeOffering = data.offering,
			routeGroups = data.groups,
			routeGroup = data.selectedGroup,
			routeTracking = data.rosterTracking,
			routeMemberships = data.memberships;
		untrack(() => {
			if (activeIdentityEpoch !== epoch) {
				activeIdentityEpoch = epoch;
				offering = null;
				groups = [];
				selectedGroup = null;
				options = null;
				tracking = null;
				preview = null;
				initialGroupSeed = null;
				editingGroup = false;
				editorSession++;
				editorBusy = false;
				weeklyPeriodTarget = 0;
				weeklyBaseline = 0;
				weeklyConflict = false;
				weeklySaving = false;
				optionsLoading = false;
				rosterLoading = false;
				pendingGroupId = '';
				contextRepairRevision++;
				contextRepairKey = '';
				contextRepairError = '';
				for (const request of [
					offeringRequest,
					groupsRequest,
					groupRequest,
					optionsRequest,
					rosterRequest
				])
					request.abort();
			}
		});
		if ([routeOffering, routeGroups, routeGroup].some((p) => routeEpoch(p) !== epoch)) return;
		const initialOffering = offeringRevision,
			initialGroups = groupsRevision,
			initialGroup = groupRevision;
		let current = true;
		untrack(() => {
			initialGroupSeed = null;
			if (activeOfferingId !== id) {
				activeOfferingId = id;
				offering = null;
				groups = [];
				selectedGroup = null;
				options = null;
				tracking = null;
				preview = null;
				editingGroup = false;
				weeklyPeriodTarget = 0;
				weeklyBaseline = 0;
			}
			offeringRequest.abort();
			groupsRequest.abort();
			groupRequest.abort();
			offeringLoading = true;
			groupsLoading = true;
			groupLoading = true;
			offeringError = '';
			groupsError = '';
			groupError = '';
		});
		const owned = () =>
			current && !disposed && activeOfferingId === id && authStore.sessionEpoch === epoch;
		void routeOffering.then((result) => {
			if (!owned()) return;
			untrack(() => {
				if (offeringRevision !== initialOffering) return;
				if (result.ok) acceptOffering(result.data);
				else offeringError = result.error;
				offeringLoading = false;
			});
		});
		void routeGroups.then((result) => {
			if (!owned()) return;
			untrack(() => {
				if (groupsRevision !== initialGroups) return;
				if (result.ok) groups = result.data;
				else groupsError = result.error;
				groupsLoading = false;
			});
		});
		void routeGroup.then((result) => {
			if (!owned()) return;
			untrack(() => {
				if (groupRevision !== initialGroup) return;
				if (result.ok) {
					selectedGroup = result.data;
					if (result.data)
						initialGroupSeed = {
							offeringId: id,
							groupId: result.data.id,
							epoch,
							tracking: routeTracking,
							memberships: routeMemberships
						};
				} else groupError = result.error;
				groupLoading = false;
			});
		});
		return () => {
			current = false;
		};
	});
	$effect(() => {
		const term = offering?.academicTermId,
			user = $authStore.user?.id;
		untrack(() => {
			if (term && user) connectTimetableSocket({ academicTermId: term, currentUserId: user });
		});
		return () => disconnectTimetableSocket();
	});
	$effect(() => {
		const id = data.offeringId;
		void $authStore.user;
		const epoch = authStore.sessionEpoch;
		let seenRevision = get(refreshTrigger);
		return refreshTrigger.subscribe((revision) => {
			if (revision === seenRevision) return;
			seenRevision = revision;
			if (!disposed && activeOfferingId === id && authStore.sessionEpoch === epoch) void refresh();
		});
	});
	onMount(() => registerDeliveryReconcile(() => void refresh()));
	onDestroy(() => {
		disposed = true;
		for (const request of [
			offeringRequest,
			groupsRequest,
			groupRequest,
			optionsRequest,
			rosterRequest
		])
			request.abort();
	});
</script>

<PageShell
	title={offering?.nameSnapshot ?? 'รายละเอียดรายการเปิดสอน'}
	description={offering?.kind === 'course'
		? offering.codeSnapshot
		: 'กลุ่มเรียน ครู และผู้เรียนปัจจุบัน'}
	{backHref}
	backPreload="tap"
>
	{#snippet actions()}{#if offering}<DeliveryResourceDeletion
				academicYearId={offering.academicYearId}
				academicTermId={offering.academicTermId}
				offeringId={offering.id}
				label={offering.nameSnapshot}
				canManage={canDelete}
				onDeleted={() => goto(backHref)}
			/>{/if}{/snippet}
	{#if contextRepairError}<div
			role="alert"
			class="space-y-2 rounded-xl border border-destructive p-3 text-sm"
		>
			{contextRepairError}<Button variant="outline" onclick={() => void repairOwnerContext(true)}
				>ลองปรับบริบทอีกครั้ง</Button
			>
		</div>{/if}
	{#if offeringLoading && !offering}<PageSkeleton
			variant="detail"
		/>{:else if offeringError && !offering}<PageState
			variant="error"
			title="ไม่สามารถเปิดรายการเปิดสอนนี้ได้"
			description={offeringError}
			actionLabel="ลองอีกครั้ง"
			onaction={loadOffering}
		/>{:else if offering}
		{#if offeringError}<p role="alert" class="text-sm text-destructive">{offeringError}</p>{/if}
		<section class="space-y-3 rounded-xl border bg-card p-4">
			<h2 class="font-semibold">คาบที่จัดจริงภาคเรียนนี้</h2>
			<p class="text-sm text-muted-foreground">
				จำนวนคาบนี้ใช้จัดตารางสอน โดยไม่เปลี่ยนค่ามาตรฐานในหลักสูตร
			</p>
			<form class="flex flex-wrap items-end gap-3" onsubmit={saveWeekly}>
				<div class="space-y-1.5">
					<Label for="delivery-weekly-target">คาบต่อสัปดาห์</Label><Input
						id="delivery-weekly-target"
						type="number"
						min="0"
						step="1"
						bind:value={weeklyPeriodTarget}
						disabled={!canManage || weeklySaving}
					/>
				</div>
				{#if canManage}<LoadingButton
						type="submit"
						loading={weeklySaving}
						loadingLabel="กำลังบันทึก"
						disabled={weeklyConflict}>บันทึกจำนวนคาบ</LoadingButton
					>{/if}
			</form>
			{#if weeklyConflict}<p role="alert" class="text-sm text-destructive">
					จำนวนคาบเปลี่ยนระหว่างแก้ไข ค่าที่คุณกรอกยังอยู่
				</p>
				<Button
					variant="outline"
					onclick={() => {
						if (offering) {
							weeklyPeriodTarget = offering.weeklyPeriodTarget;
							weeklyBaseline = weeklyPeriodTarget;
							weeklyConflict = false;
						}
					}}>ใช้ค่าล่าสุด</Button
				>{/if}{#if weeklyError}<p role="alert" class="text-sm text-destructive">
					{weeklyError}
				</p>{/if}
		</section>
		{#if canManage}<DeliveryBulkGroupSetupDialog
				{offering}
				{groups}
				canManage={canManage && !editingGroup}
				onRequestManagementOptions={loadOptions}
				onApplied={bulkApplied}
				onReload={loadGroups}
			/>{/if}
		<section
			class="relative"
			aria-busy={groupsLoading}
			data-testid={groups.length || !groupsLoading ? 'delivery-groups-ready' : undefined}
		>
			{#if groupsLoading && !groups.length}<PageSkeleton
					variant="cards"
					rows={2}
				/>{:else if groupsError && !groups.length}<PageState
					variant="error"
					title="โหลดกลุ่มเรียนไม่สำเร็จ"
					description={groupsError}
					actionLabel="ลองอีกครั้ง"
					onaction={() => void loadGroups()}
				/>{:else}{#key offering.id}<LearningGroupList
						{offering}
						{groups}
						selectedGroupId={selectedGroup?.id}
						{canManage}
						onSelect={selectGroup}
						onRequestManagementOptions={loadOptions}
						onCreate={createGroup}
					/>{/key}{/if}
		</section>
		{#if groupLoading && !selectedGroup}<PageSkeleton
				variant="detail"
			/>{:else if groupError && !selectedGroup}<PageState
				variant="error"
				title="โหลดกลุ่มเรียนไม่สำเร็จ"
				description={groupError}
				actionLabel="ลองอีกครั้ง"
				onaction={() => void loadGroups()}
			/>{:else if selectedGroup}
			{#key selectedGroup.id}
				<div class="flex flex-wrap items-center justify-between gap-3">
					<h2 class="text-lg font-semibold">{selectedGroup.name}</h2>
					<div class="flex flex-wrap gap-2">
						{#if canManage}<Button
								variant="outline"
								disabled={optionsLoading}
								bind:ref={editorTrigger}
								onclick={openGroupEditor}>แก้ไขกลุ่มและครู</Button
							>{#if editingGroup}<Button
									variant="outline"
									disabled={editorBusy}
									onclick={cancelGroupEditor}>ยกเลิกการแก้ไขกลุ่ม</Button
								>{/if}{/if}<DeliveryResourceDeletion
							academicYearId={offering.academicYearId}
							academicTermId={offering.academicTermId}
							offeringId={offering.id}
							groupId={selectedGroup.id}
							label={selectedGroup.name}
							canManage={canDelete}
							onDeleted={async () => {
								selectedGroup = null;
								await loadGroups();
							}}
						/>
					</div>
				</div>
				{#if editingGroup}{#if optionsLoading && !options}<PageSkeleton
							variant="detail"
						/>{:else if optionsError && !options}<PageState
							variant="error"
							title="โหลดตัวเลือกไม่สำเร็จ"
							description={optionsError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadOptions}
						/>{:else if options}<LearningGroupEditor
							{offering}
							group={selectedGroup}
							managementOptions={options}
							{canManage}
							onReload={async () => {
								if (!selectedGroup || !canManage) throw new Error('กลุ่มนี้ไม่พร้อมแก้ไข');
								const id = selectedGroup.id,
									current = admission(id);
								const latest = await getLearningGroup(id);
								if (current()) acceptGroup(latest);
								return latest;
							}}
							onBusyChange={editorBusyObserver()}
							onSaveGroup={saveGroup}
							onReplaceTeachers={saveTeachers}
							onReplaceHomerooms={saveHomerooms}
						/>{/if}{/if}
				<RosterTrackingPanel
					group={selectedGroup}
					{canManage}
					bind:tracking
					initialTracking={initialGroupSeedIsCurrent ? initialGroupSeed?.tracking : null}
					onUpdated={trackingUpdated}
				/>
				<DatedRosterMemberships
					group={selectedGroup}
					{canManage}
					initialMemberships={initialGroupSeedIsCurrent ? initialGroupSeed?.memberships : null}
					onGroupChanged={async () => {
						if (selectedGroup) await selectGroup(selectedGroup);
					}}
				/>
				<RosterPreviewPanel
					group={selectedGroup}
					{preview}
					loading={rosterLoading}
					stale={previewStale}
					{canManage}
					onRefresh={refreshRoster}
					onApply={applyRoster}
				/>
			{/key}
		{/if}
	{/if}
</PageShell>
