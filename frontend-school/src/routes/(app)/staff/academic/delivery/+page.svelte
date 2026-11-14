<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { onDestroy, onMount, untrack } from 'svelte';
	import { get } from 'svelte/store';
	import {
		getDeliveryWorkspace,
		getHomeroomDeliveryWorkspace,
		type DeliveryWorkspace,
		type HomeroomDeliveryWorkspace as HomeroomWorkspace
	} from '#lib/api/learning-delivery.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { buildDeliveryAlignmentFacts } from '#lib/academic/homeroom-delivery.js';
	import {
		buildCurriculumPreparationTarget,
		type CurriculumPreparationFocus,
		type CurriculumPreparationTarget
	} from '#lib/academic/synchronized-activity-delivery.js';
	import type { DeliveryOfferingAddKind } from '#lib/academic/learning-delivery-page.js';
	import {
		deliveryOverviewStateHref,
		deliveryOverviewReturnHref,
		readDeliveryOverviewState,
		type DeliveryOverviewState
	} from '#lib/navigation/learning-delivery.js';
	import {
		connectTimetableSocket,
		disconnectTimetableSocket,
		refreshTrigger
	} from '#lib/stores/timetable-socket.js';
	import { registerDeliveryReconcile } from '#lib/academic/delivery-reconcile.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import OfferingCreateDialog from '#lib/components/learning-delivery/OfferingCreateDialog.svelte';
	import DeliveryOverviewTable from '#lib/components/learning-delivery/DeliveryOverviewTable.svelte';
	import HomeroomDeliveryWorkspace from '#lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte';
	import DeliveryAlignmentSummary from '#lib/components/learning-delivery/DeliveryAlignmentSummary.svelte';
	import DeliveryMissingRequirements from '#lib/components/learning-delivery/DeliveryMissingRequirements.svelte';
	let { data }: { data: PageData } = $props();
	const academicYearId = $derived(data.context?.academicYearId ?? null),
		academicTermId = $derived(data.context?.academicTermId ?? null);
	const overviewUrl = $derived(new URL((page.shallow?.url ?? page.url).href));
	const viewState = $derived(readDeliveryOverviewState(overviewUrl));
	const canManage = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	let workspace = $state.raw<HomeroomWorkspace | null>(null),
		overview = $state.raw<DeliveryWorkspace | null>(null);
	let homeroomLoading = $state(false),
		overviewLoading = $state(false),
		homeroomError = $state(''),
		overviewError = $state('');
	const homeroomRequest = new LatestRequest(),
		overviewRequest = new LatestRequest();
	const facts = $derived(workspace ? buildDeliveryAlignmentFacts(workspace) : null);
	const returnTo = $derived(
		data.context ? deliveryOverviewReturnHref(overviewUrl, data.context) : ''
	);
	let offeringDialog = $state<{
		openManual: (kind: DeliveryOfferingAddKind) => Promise<void>;
		openCurriculumPreparation: (target: CurriculumPreparationTarget) => Promise<void>;
	}>();
	let activeContext = '',
		disposed = false,
		homeroomRevision = 0,
		overviewRevision = 0;
	const routeEpochs = new WeakMap<object, number>();
	function routeEpoch(promise: object) {
		const known = routeEpochs.get(promise);
		if (known !== undefined) return known;
		const epoch = authStore.sessionEpoch;
		routeEpochs.set(promise, epoch);
		return epoch;
	}
	function admission() {
		const context = activeContext,
			epoch = authStore.sessionEpoch;
		return () => !disposed && activeContext === context && authStore.sessionEpoch === epoch;
	}
	const contextMismatch = 'ข้อมูลไม่ตรงกับปีการศึกษาและภาคเรียนที่เลือก กรุณาโหลดอีกครั้ง';
	function matchesContext(
		value: Pick<DeliveryWorkspace, 'academicYearId' | 'academicTermId'>,
		yearId: string,
		termId: string
	): boolean {
		return value.academicYearId === yearId && value.academicTermId === termId;
	}
	async function loadHomerooms() {
		if (!academicYearId || !academicTermId) return;
		const yearId = academicYearId,
			termId = academicTermId,
			current = admission(),
			ticket = homeroomRequest.begin();
		homeroomRevision++;
		homeroomLoading = true;
		homeroomError = '';
		try {
			const loaded = await getHomeroomDeliveryWorkspace(academicYearId, academicTermId, {
				signal: ticket.signal
			});
			if (current() && homeroomRequest.isCurrent(ticket.revision)) {
				if (!matchesContext(loaded, yearId, termId)) {
					workspace = null;
					throw new Error(contextMismatch);
				}
				workspace = loaded;
			}
		} catch (error) {
			if (current() && homeroomRequest.isCurrent(ticket.revision) && !isAbortError(error))
				homeroomError = error instanceof Error ? error.message : 'โหลดภาพรวมไม่สำเร็จ';
		} finally {
			if (current() && homeroomRequest.isCurrent(ticket.revision)) homeroomLoading = false;
		}
	}
	async function loadOverview() {
		if (!academicYearId || !academicTermId) return;
		const yearId = academicYearId,
			termId = academicTermId,
			current = admission(),
			ticket = overviewRequest.begin();
		overviewRevision++;
		overviewLoading = true;
		overviewError = '';
		try {
			const loaded = await getDeliveryWorkspace(academicYearId, academicTermId, {
				signal: ticket.signal
			});
			if (current() && overviewRequest.isCurrent(ticket.revision)) {
				if (!matchesContext(loaded, yearId, termId)) {
					overview = null;
					throw new Error(contextMismatch);
				}
				overview = loaded;
			}
		} catch (error) {
			if (current() && overviewRequest.isCurrent(ticket.revision) && !isAbortError(error))
				overviewError = error instanceof Error ? error.message : 'โหลดรายการเปิดสอนไม่สำเร็จ';
		} finally {
			if (current() && overviewRequest.isCurrent(ticket.revision)) overviewLoading = false;
		}
	}
	async function refresh() {
		await Promise.all([
			loadHomerooms(),
			viewState.view === 'offerings' ? loadOverview() : Promise.resolve()
		]);
	}
	function updateViewState(patch: Partial<DeliveryOverviewState>) {
		void goto(deliveryOverviewStateHref(overviewUrl, patch), {
			shallow: true,
			replace: true,
			state: page.state
		});
	}
	async function changeView(value: string) {
		if (value !== 'homerooms' && value !== 'offerings') return;
		updateViewState({ view: value });
		if (value === 'offerings' && !overview && !overviewLoading) await loadOverview();
	}
	async function prepare(focus: CurriculumPreparationFocus) {
		if (!workspace || !canManage) return;
		const target = buildCurriculumPreparationTarget(workspace, focus);
		if (target) await offeringDialog?.openCurriculumPreparation(target);
	}
	$effect.pre(() => {
		const context = data.context,
			key = context ? `${context.academicYearId}:${context.academicTermId}` : '',
			epoch = authStore.sessionEpoch,
			homerooms = data.homerooms,
			routeOverview = data.overview;
		if (
			(homerooms && routeEpoch(homerooms) !== epoch) ||
			(routeOverview && routeEpoch(routeOverview) !== epoch)
		)
			return;
		const initialHomeroomRevision = homeroomRevision,
			initialOverviewRevision = overviewRevision;
		let current = true;
		untrack(() => {
			if (activeContext !== key) {
				activeContext = key;
				workspace = null;
				overview = null;
			}
			homeroomRequest.abort();
			overviewRequest.abort();
			homeroomLoading = Boolean(homerooms);
			overviewLoading = Boolean(routeOverview);
			homeroomError = '';
			overviewError = '';
		});
		if (homerooms)
			void homerooms.then((result) => {
				if (!current || disposed || activeContext !== key || authStore.sessionEpoch !== epoch)
					return;
				untrack(() => {
					if (homeroomRevision !== initialHomeroomRevision) return;
					if (result.ok) {
						if (
							context &&
							matchesContext(result.data, context.academicYearId, context.academicTermId)
						)
							workspace = result.data;
						else {
							workspace = null;
							homeroomError = contextMismatch;
						}
					} else homeroomError = result.error;
					homeroomLoading = false;
				});
			});
		if (routeOverview)
			void routeOverview.then((result) => {
				if (!current || disposed || activeContext !== key || authStore.sessionEpoch !== epoch)
					return;
				untrack(() => {
					if (overviewRevision !== initialOverviewRevision) return;
					if (result.ok) {
						if (
							context &&
							matchesContext(result.data, context.academicYearId, context.academicTermId)
						)
							overview = result.data;
						else {
							overview = null;
							overviewError = contextMismatch;
						}
					} else overviewError = result.error;
					overviewLoading = false;
				});
			});
		return () => {
			current = false;
		};
	});
	$effect(() => {
		const term = academicTermId,
			user = $authStore.user?.id;
		untrack(() => {
			if (term && user) connectTimetableSocket({ academicTermId: term, currentUserId: user });
		});
		return () => disconnectTimetableSocket();
	});
	$effect(() => {
		const key = `${academicYearId}:${academicTermId}`;
		void $authStore.user;
		const epoch = authStore.sessionEpoch;
		let seenRevision = get(refreshTrigger);
		return refreshTrigger.subscribe((revision) => {
			if (revision === seenRevision) return;
			seenRevision = revision;
			if (
				!disposed &&
				academicYearId &&
				academicTermId &&
				activeContext === key &&
				authStore.sessionEpoch === epoch
			)
				void refresh();
		});
	});
	onMount(() => registerDeliveryReconcile(() => void refresh()));
	onDestroy(() => {
		disposed = true;
		homeroomRequest.abort();
		overviewRequest.abort();
	});
</script>

<PageShell
	title="รายวิชาและกิจกรรมที่เปิดสอน"
	description="จัดรายวิชา กลุ่มเรียน ครู และผู้เรียนของภาคเรียนปัจจุบัน"
>
	{#snippet actions()}{#if academicTermId && canManage}{#key academicTermId}<OfferingCreateDialog
					bind:this={offeringDialog}
					{academicTermId}
					{returnTo}
					onCreated={() => void refresh()}
					onApplied={refresh}
				/>{/key}{/if}{/snippet}
	{#if !academicYearId || !academicTermId}<PageState
			variant="empty"
			title="เลือกปีการศึกษาและภาคเรียนก่อน"
			description="เลือกบริบทจากแถบด้านบนเพื่อจัดการรายการเปิดสอน"
		/>
	{:else}
		<DeliveryAlignmentSummary
			{workspace}
			{facts}
			loading={homeroomLoading}
			error={homeroomError}
			preparing={false}
			{canManage}
			{returnTo}
			onRetry={loadHomerooms}
			onPrepare={prepare}
		/>
		<Tabs.Root value={viewState.view} onValueChange={changeView}>
			<Tabs.List class="grid w-full grid-cols-2 sm:w-[430px]"
				><Tabs.Trigger value="homerooms">มุมมองรายห้อง</Tabs.Trigger><Tabs.Trigger value="offerings"
					>มุมมองรายวิชา/กิจกรรม</Tabs.Trigger
				></Tabs.List
			>
			<Tabs.Content value="homerooms" class="mt-4"
				><section
					class="relative"
					aria-label="มุมมองรายห้อง"
					aria-busy={homeroomLoading}
					data-testid={workspace ? 'delivery-homerooms-ready' : undefined}
				>
					{#if homeroomLoading && !workspace}<PageSkeleton
							variant="cards"
							rows={4}
						/>{:else if homeroomError && !workspace}<PageState
							variant="error"
							title="โหลดภาพรวมรายห้องไม่สำเร็จ"
							description={homeroomError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadHomerooms}
						/>{:else if workspace}<HomeroomDeliveryWorkspace
							{workspace}
							{canManage}
							filters={viewState}
							{returnTo}
							onFiltersChange={updateViewState}
							onPrepareCurriculumRequirement={prepare}
							onAddOffering={(kind) => void offeringDialog?.openManual(kind)}
						/>{/if}
				</section></Tabs.Content
			>
			<Tabs.Content value="offerings" class="mt-4"
				><section
					class="relative"
					aria-label="มุมมองรายวิชาและกิจกรรม"
					aria-busy={overviewLoading}
					data-testid={overview ? 'delivery-offerings-ready' : undefined}
				>
					{#if overviewLoading && !overview}<PageSkeleton
							variant="table"
							rows={6}
						/>{:else if overviewError && !overview}<PageState
							variant="error"
							title="โหลดรายการเปิดสอนไม่สำเร็จ"
							description={overviewError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadOverview}
						/>{:else if overview}<DeliveryOverviewTable
							workspace={overview}
							comparisonWorkspace={workspace}
							alignmentFacts={facts}
							filters={viewState}
							{returnTo}
							onFiltersChange={updateViewState}
						/>{/if}
					{#if overviewError && overview}<div
							role="alert"
							class="my-3 rounded-lg border border-destructive/40 p-3 text-sm"
						>
							{overviewError}<Button variant="outline" onclick={loadOverview}>ลองอีกครั้ง</Button>
						</div>{/if}
				</section>
				<DeliveryMissingRequirements
					{workspace}
					{facts}
					loading={homeroomLoading}
					error={homeroomError}
					preparing={false}
					{canManage}
					filters={viewState}
					showFilters={!overview?.offerings.length}
					onFiltersChange={updateViewState}
					onRetry={loadHomerooms}
					onPrepare={prepare}
				/></Tabs.Content
			>
		</Tabs.Root>
	{/if}
</PageShell>
