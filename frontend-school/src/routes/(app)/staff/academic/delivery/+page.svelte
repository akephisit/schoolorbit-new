<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/state';
	import { replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { untrack } from 'svelte';
	import {
		selectAcademicTermChangeSetSummary,
		summarizeAcademicTermChangeSet,
		type LearningDeliveryRefreshScope
	} from '$lib/academic/learning-delivery-page';
	import {
		buildSynchronizedActivityPreparationTarget,
		type SynchronizedActivityPreparationTarget
	} from '$lib/academic/synchronized-activity-delivery';
	import {
		getAcademicTermChangeSet,
		getHomeroomDeliveryWorkspace,
		getLearningDeliveryOverview,
		listAcademicTermChangeSets,
		type AcademicTermChangeSet,
		type AcademicTermChangeSetSummary,
		type HomeroomDeliveryWorkspace as HomeroomWorkspace,
		type LearningDeliveryOverview,
		type LearningOfferingOverviewItem
	} from '$lib/api/learning-delivery';
	import { includeTimetableVersionOffering } from '$lib/api/timetable';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import {
		AcademicPrerequisiteNotice,
		type AcademicPrerequisite
	} from '$lib/components/academic-workflow';
	import AcademicChangeSetDialog from '$lib/components/learning-delivery/AcademicChangeSetDialog.svelte';
	import AcademicChangeSetPanel from '$lib/components/learning-delivery/AcademicChangeSetPanel.svelte';
	import HomeroomDeliveryWorkspace from '$lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte';
	import OfferingCreateDialog from '$lib/components/learning-delivery/OfferingCreateDialog.svelte';
	import OfferingOverviewTable from '$lib/components/learning-delivery/OfferingOverviewTable.svelte';
	import * as Select from '$lib/components/ui/select';
	import * as Tabs from '$lib/components/ui/tabs';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let { data }: { data: PageData } = $props();
	const academicYearId = $derived(data.context?.academicYearId ?? null);
	const academicTermId = $derived(data.context?.academicTermId ?? null);
	const homeroomRequest = new LatestRequest();
	const changeSetSummaryRequest = new LatestRequest();
	const changeSetRequest = new LatestRequest();
	const overviewRequest = new LatestRequest();
	let workspace = $state.raw<HomeroomWorkspace | null>(null);
	let overview = $state.raw<LearningDeliveryOverview | null>(null);
	let changeSets = $state.raw<AcademicTermChangeSetSummary[]>([]);
	let activeChangeSet = $state.raw<AcademicTermChangeSet | null>(null);
	let selectedChangeSetId = $state('');
	let homeroomLoading = $state(false);
	let changeSetSummaryLoading = $state(false);
	let changeSetLoading = $state(false);
	let overviewLoading = $state(false);
	let homeroomError = $state('');
	let changeSetSummaryError = $state('');
	let changeSetError = $state('');
	let errorMessage = $state('');
	let viewMode = $state<'homerooms' | 'offerings'>('homerooms');
	let offeringDialog = $state<{
		openCurriculumPreparation: (
			target: SynchronizedActivityPreparationTarget,
			timetableVersionId?: string | null
		) => Promise<void>;
	}>();
	let timetableRevisionDialog = $state<{ openDialog: () => void }>();
	let pendingTimetableAction = $state.raw<
		{ kind: 'activate'; catalogVersionId: string } | { kind: 'include' } | null
	>(null);
	let initialKind = $derived<'all' | 'activity'>(
		page.url.searchParams.get('kind') === 'activity' ? 'activity' : 'all'
	);
	let canManage = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	let canManageTimetable = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_ASSIGNED
		)
	);
	let items = $derived(overview?.offerings ?? []);
	let activeChangeSetSummary = $derived(
		changeSets.find((changeSet) => changeSet.id === selectedChangeSetId) ?? null
	);
	let activeChangeSetLabel = $derived(
		activeChangeSetSummary
			? formatChangeSetOption(activeChangeSetSummary)
			: 'เลือกชุดการเปลี่ยนแปลง'
	);

	const missingTermPrerequisite: AcademicPrerequisite = {
		key: 'academic-term',
		status: 'missing',
		title: 'เลือกปีการศึกษาและภาคเรียนก่อน',
		description: 'มุมมองรายห้อง รายการเปิดสอน กลุ่ม ครู และตาราง แยกกันในแต่ละภาคเรียน',
		actionLabel: 'ไปตั้งค่าปีและภาคเรียน',
		href: '/staff/academic/core'
	};
	const noOfferingPrerequisite: AcademicPrerequisite = {
		key: 'learning-offerings',
		status: 'warning',
		title: 'ภาคเรียนนี้ยังไม่มีรายการเปิดสอน',
		description: 'นำรายวิชาและกิจกรรมจากหลักสูตรมาใช้ หรือเพิ่มรายการเฉพาะภาคเรียนนี้ได้',
		actionLabel: 'ตรวจหลักสูตรและแผนการเรียน',
		href: '/staff/academic/curricula'
	};

	function formatDate(value: string): string {
		return new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium' }).format(
			new Date(`${value}T00:00:00`)
		);
	}

	function formatChangeSetOption(changeSet: AcademicTermChangeSetSummary): string {
		const status =
			changeSet.status === 'draft'
				? 'แบบร่าง'
				: changeSet.status === 'published'
					? 'เผยแพร่แล้ว'
					: 'ยกเลิกแล้ว';
		return `${formatDate(changeSet.effectiveFrom)} · ${status} · ${changeSet.reason}`;
	}

	function updateSelectedChangeSetUrl(id: string) {
		const url = new URL(page.url);
		if (id) url.searchParams.set('changeSetId', id);
		else url.searchParams.delete('changeSetId');
		replaceState(resolve(`/staff/academic/delivery?${url.searchParams.toString()}`), page.state);
	}

	function applyChangeSetSummaries(loaded: AcademicTermChangeSetSummary[]) {
		changeSets = loaded;
		const selected = selectAcademicTermChangeSetSummary(
			loaded,
			page.url.searchParams.get('changeSetId')?.trim() || selectedChangeSetId
		);
		selectedChangeSetId = selected?.id ?? '';
		if (!selected) activeChangeSet = null;
		return selected;
	}

	async function loadHomerooms() {
		if (!academicYearId || !academicTermId) return;
		const { revision, signal } = homeroomRequest.begin();
		homeroomLoading = true;
		homeroomError = '';
		try {
			const result = await getHomeroomDeliveryWorkspace(academicYearId, academicTermId, {
				timetableVersionId: page.url.searchParams.get('timetableVersionId') ?? undefined,
				signal
			});
			if (homeroomRequest.isCurrent(revision)) workspace = result;
		} catch (error) {
			if (isAbortError(error)) return;
			if (homeroomRequest.isCurrent(revision)) {
				homeroomError = error instanceof Error ? error.message : 'โหลดภาพรวมรายห้องไม่สำเร็จ';
			}
		} finally {
			if (homeroomRequest.isCurrent(revision)) homeroomLoading = false;
		}
	}

	async function loadSelectedChangeSet(id: string, updateUrl = true) {
		if (!id || !academicTermId) {
			activeChangeSet = null;
			changeSetLoading = false;
			return;
		}
		const { revision, signal } = changeSetRequest.begin();
		selectedChangeSetId = id;
		activeChangeSet = null;
		changeSetLoading = true;
		changeSetError = '';
		if (updateUrl) updateSelectedChangeSetUrl(id);
		try {
			const result = await getAcademicTermChangeSet(id, { signal });
			if (result.academicTermId !== academicTermId) {
				throw new Error('ชุดการเปลี่ยนแปลงไม่อยู่ในภาคเรียนที่เลือก');
			}
			if (changeSetRequest.isCurrent(revision)) activeChangeSet = result;
		} catch (error) {
			if (isAbortError(error)) return;
			if (changeSetRequest.isCurrent(revision)) {
				changeSetError =
					error instanceof Error ? error.message : 'โหลดรายละเอียดชุดการเปลี่ยนแปลงไม่สำเร็จ';
			}
		} finally {
			if (changeSetRequest.isCurrent(revision)) changeSetLoading = false;
		}
	}

	async function loadChangeSetSummaries() {
		if (!academicTermId) return;
		const { revision, signal } = changeSetSummaryRequest.begin();
		changeSetSummaryLoading = true;
		changeSetSummaryError = '';
		try {
			const result = await listAcademicTermChangeSets(academicTermId, { signal });
			if (!changeSetSummaryRequest.isCurrent(revision)) return;
			const selected = applyChangeSetSummaries(result);
			if (selected) await loadSelectedChangeSet(selected.id, false);
		} catch (error) {
			if (isAbortError(error)) return;
			if (changeSetSummaryRequest.isCurrent(revision)) {
				changeSetSummaryError =
					error instanceof Error ? error.message : 'โหลดรายการเปลี่ยนแปลงกลางภาคไม่สำเร็จ';
			}
		} finally {
			if (changeSetSummaryRequest.isCurrent(revision)) changeSetSummaryLoading = false;
		}
	}

	async function loadOverview(termId: string) {
		const { revision, signal } = overviewRequest.begin();
		overviewLoading = true;
		try {
			const result = await getLearningDeliveryOverview(termId, { signal });
			if (overviewRequest.isCurrent(revision)) overview = result;
		} catch (error) {
			if (isAbortError(error)) return;
			if (overviewRequest.isCurrent(revision))
				errorMessage = error instanceof Error ? error.message : 'โหลดมุมมองรายวิชาไม่สำเร็จ';
		} finally {
			if (overviewRequest.isCurrent(revision)) overviewLoading = false;
		}
	}

	async function ensureOverview() {
		if (!academicTermId || overview || overviewLoading) return;
		await loadOverview(academicTermId);
	}

	async function refreshDeliveryRegions(refreshOverview = viewMode === 'offerings') {
		const requests: Promise<void>[] = [loadHomerooms()];
		if (refreshOverview && academicTermId) requests.push(loadOverview(academicTermId));
		await Promise.all(requests);
	}

	function changeViewMode(value: string) {
		viewMode = value === 'offerings' ? 'offerings' : 'homerooms';
		if (viewMode === 'offerings' && academicTermId && !overview && !overviewLoading)
			void loadOverview(academicTermId);
	}

	function addCreated(item: LearningOfferingOverviewItem) {
		if (!overview) {
			overview = { academicTermId: item.offering.academicTermId, offerings: [item] };
		} else {
			overview = {
				...overview,
				offerings: [...overview.offerings, item].sort((left, right) =>
					left.offering.codeSnapshot.localeCompare(right.offering.codeSnapshot, 'th-TH', {
						numeric: true
					})
				)
			};
		}
		void loadHomerooms();
	}

	function prepareSynchronizedActivity(catalogVersionId: string) {
		if (!workspace || !offeringDialog) return;
		if (workspace.timetableVersionStatus === 'published') {
			pendingTimetableAction = { kind: 'activate', catalogVersionId };
			timetableRevisionDialog?.openDialog();
			return;
		}
		const target = buildSynchronizedActivityPreparationTarget(workspace, catalogVersionId);
		if (!target) return;
		void offeringDialog.openCurriculumPreparation(
			target,
			workspace.timetableVersionStatus === 'draft' ? workspace.timetableVersionId : null
		);
	}

	async function includeOfferingInTimetable(offeringId: string) {
		if (!workspace || !academicYearId || !academicTermId) return;
		if (workspace.timetableVersionStatus === 'published') {
			pendingTimetableAction = { kind: 'include' };
			timetableRevisionDialog?.openDialog();
			return;
		}
		if (workspace.timetableVersionStatus !== 'draft' || !workspace.timetableVersionId) {
			errorMessage = 'ยังไม่มีรุ่นตารางแบบร่างสำหรับเพิ่มรายการเปิดสอน';
			return;
		}
		errorMessage = '';
		try {
			await includeTimetableVersionOffering(workspace.timetableVersionId, {
				learningOfferingId: offeringId
			});
			await loadHomerooms();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'เพิ่มรายการเข้ารุ่นตารางไม่สำเร็จ';
		}
	}

	async function handleTimetableRevisionCreated(created: AcademicTermChangeSet) {
		const pending = pendingTimetableAction;
		pendingTimetableAction = null;
		addChangeSet(created);
		if (!academicYearId || !academicTermId) return;
		const url = new URL(page.url);
		url.searchParams.set('timetableVersionId', created.targetTimetableVersionId);
		url.searchParams.set('changeSetId', created.id);
		replaceState(resolve(`/staff/academic/delivery?${url.searchParams.toString()}`), page.state);
		await loadHomerooms();
		if (pending?.kind !== 'activate' || !workspace || !offeringDialog) return;
		const target = buildSynchronizedActivityPreparationTarget(workspace, pending.catalogVersionId);
		if (!target) return;
		await offeringDialog.openCurriculumPreparation(target, created.targetTimetableVersionId);
	}

	function addChangeSet(created: AcademicTermChangeSet) {
		const summary = summarizeAcademicTermChangeSet(created);
		changeSets = [summary, ...changeSets.filter((changeSet) => changeSet.id !== created.id)];
		selectedChangeSetId = created.id;
		activeChangeSet = created;
		updateSelectedChangeSetUrl(created.id);
	}

	async function updateChangeSet(
		updated: AcademicTermChangeSet,
		refreshScope: LearningDeliveryRefreshScope = 'local'
	) {
		selectedChangeSetId = updated.id;
		activeChangeSet = updated;
		const summary = summarizeAcademicTermChangeSet(updated);
		changeSets = changeSets
			.map((changeSet) => (changeSet.id === updated.id ? summary : changeSet))
			.sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
		if (updated.items.length > 0 && !overview && academicTermId) {
			await loadOverview(academicTermId);
		}
		if (refreshScope === 'homerooms') await loadHomerooms();
	}

	$effect(() => {
		const routeResult = data.homerooms;
		const { revision } = homeroomRequest.begin();
		untrack(() => {
			workspace = null;
			homeroomLoading = Boolean(routeResult);
			homeroomError = '';
		});
		if (routeResult) {
			void routeResult.then((result) => {
				if (!homeroomRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) workspace = result.data;
					else homeroomError = result.error;
					homeroomLoading = false;
				});
			});
		}
		return () => {
			if (homeroomRequest.isCurrent(revision)) homeroomRequest.abort();
		};
	});

	$effect(() => {
		const routeResult = data.changeSetSummaries;
		const { revision } = changeSetSummaryRequest.begin();
		untrack(() => {
			changeSets = [];
			changeSetSummaryLoading = Boolean(routeResult);
			changeSetSummaryError = '';
		});
		if (routeResult) {
			void routeResult.then((result) => {
				if (!changeSetSummaryRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) applyChangeSetSummaries(result.data);
					else changeSetSummaryError = result.error;
					changeSetSummaryLoading = false;
				});
			});
		}
		return () => {
			if (changeSetSummaryRequest.isCurrent(revision)) changeSetSummaryRequest.abort();
		};
	});

	$effect(() => {
		const routeResult = data.selectedChangeSet;
		const { revision } = changeSetRequest.begin();
		untrack(() => {
			activeChangeSet = null;
			changeSetLoading = Boolean(routeResult);
			changeSetError = '';
		});
		if (routeResult) {
			void routeResult.then((result) => {
				if (!changeSetRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) {
						activeChangeSet = result.data;
						if (result.data) selectedChangeSetId = result.data.id;
					} else changeSetError = result.error;
					changeSetLoading = false;
				});
			});
		}
		return () => {
			if (changeSetRequest.isCurrent(revision)) changeSetRequest.abort();
		};
	});

	$effect(() => {
		const termId = academicTermId;
		overviewRequest.abort();
		untrack(() => {
			overview = null;
			overviewLoading = false;
			errorMessage = '';
			if (!termId) viewMode = 'homerooms';
		});
		return () => overviewRequest.abort();
	});
</script>

<PageShell
	title="จัดการการเปิดสอน"
	description="ตรวจจากห้องประจำชั้นว่าเรียนอะไรบ้าง แล้วจัดรายการเปิดสอน กลุ่ม ครู และตารางให้ครบ"
>
	{#snippet actions()}
		{#if canManage && academicTermId}
			<OfferingCreateDialog
				bind:this={offeringDialog}
				{academicTermId}
				onCreated={addCreated}
				onApplied={() => refreshDeliveryRegions(true)}
				defaultTimetableVersionId={workspace?.timetableVersionStatus === 'draft'
					? workspace.timetableVersionId
					: null}
			/>
			<AcademicChangeSetDialog {academicTermId} onCreated={addChangeSet} />
			{#if canManageTimetable}
				<AcademicChangeSetDialog
					bind:this={timetableRevisionDialog}
					{academicTermId}
					purpose="timetable_revision"
					showTrigger={false}
					onCreated={handleTimetableRevisionCreated}
				/>
			{/if}
		{/if}
	{/snippet}

	{#if !academicYearId || !academicTermId}
		<AcademicPrerequisiteNotice prerequisite={missingTermPrerequisite} />
	{:else}
		<div class="space-y-4">
			{#if changeSetSummaryLoading && changeSets.length === 0 && !activeChangeSet}
				<PageSkeleton variant="detail" />
			{:else if changeSetSummaryError && changeSets.length === 0 && !activeChangeSet}
				<PageState
					variant="error"
					title="โหลดรายการเปลี่ยนแปลงกลางภาคไม่สำเร็จ"
					description={changeSetSummaryError}
					actionLabel="ลองอีกครั้ง"
					onaction={loadChangeSetSummaries}
				/>
			{:else}
				{#if changeSets.length > 1}
					<section
						class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card p-3"
					>
						<div>
							<p class="text-sm font-medium">ชุดการเปลี่ยนแปลงกลางภาค</p>
							<p class="text-xs text-muted-foreground">
								เลือกดูแบบร่างที่กำลังทำหรือประวัติที่เผยแพร่และยกเลิกแล้ว
							</p>
						</div>
						<Select.Root
							type="single"
							value={selectedChangeSetId}
							onValueChange={(value) => void loadSelectedChangeSet(value)}
						>
							<Select.Trigger class="w-full sm:w-[430px]">
								<span class="truncate">{activeChangeSetLabel}</span>
							</Select.Trigger>
							<Select.Content>
								{#each changeSets as changeSet (changeSet.id)}
									<Select.Item value={changeSet.id}>
										{formatChangeSetOption(changeSet)}
									</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</section>
				{/if}
				{#if changeSetLoading && !activeChangeSet}
					<PageSkeleton variant="detail" />
				{:else if changeSetError && !activeChangeSet}
					<PageState
						variant="error"
						title="โหลดรายละเอียดชุดการเปลี่ยนแปลงไม่สำเร็จ"
						description={changeSetError}
						actionLabel="ลองอีกครั้ง"
						onaction={() => loadSelectedChangeSet(selectedChangeSetId, false)}
					/>
				{:else if activeChangeSet}
					{#key activeChangeSet.id}
						<AcademicChangeSetPanel
							changeSet={activeChangeSet}
							offerings={items}
							{canManage}
							ensureOfferings={ensureOverview}
							initialTeacherChangeItemId={page.url.searchParams.get('teacherChangeItemId') ?? ''}
							onChanged={updateChangeSet}
						/>
					{/key}
				{:else if canManage}
					<section
						class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-dashed border-amber-500/35 bg-amber-500/5 p-3 text-sm"
					>
						<div>
							<p class="font-medium text-amber-900">เมื่อเปิดสอนแล้วและต้องเปลี่ยนกลางภาค</p>
							<p class="text-xs text-muted-foreground">
								ใช้ปุ่ม “เพิ่ม/ปรับ/หยุดกลางภาค” ด้านบน ระบบจะแยกรุ่นตารางและเก็บประวัติเดิมให้
							</p>
						</div>
					</section>
				{/if}
			{/if}
			<Tabs.Root value={viewMode} onValueChange={changeViewMode}>
				<Tabs.List class="grid w-full grid-cols-2 sm:w-[430px]">
					<Tabs.Trigger value="homerooms">มุมมองรายห้อง</Tabs.Trigger>
					<Tabs.Trigger value="offerings">มุมมองรายวิชา/กิจกรรม</Tabs.Trigger>
				</Tabs.List>
				<Tabs.Content value="homerooms" class="mt-4">
					{#if homeroomLoading && !workspace}
						<PageSkeleton variant="cards" rows={4} />
					{:else if homeroomError && !workspace}
						<PageState
							variant="error"
							title="โหลดภาพรวมรายห้องไม่สำเร็จ"
							description={homeroomError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadHomerooms}
						/>
					{:else if workspace}
						<HomeroomDeliveryWorkspace
							{workspace}
							{canManage}
							{canManageTimetable}
							onPrepareSynchronizedActivity={prepareSynchronizedActivity}
							onIncludeOfferingInTimetable={includeOfferingInTimetable}
						/>
					{/if}
				</Tabs.Content>
				<Tabs.Content value="offerings" class="mt-4">
					{#if overviewLoading && !overview}
						<PageSkeleton variant="table" rows={6} />
					{:else if items.length === 0}
						<AcademicPrerequisiteNotice prerequisite={noOfferingPrerequisite} />
					{:else}
						<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
							<div
								class="flex flex-wrap items-start justify-between gap-4 border-b bg-muted/25 p-4"
							>
								<div>
									<h2 class="font-semibold">รายการเปิดสอนของภาคเรียน</h2>
									<p class="mt-1 text-sm text-muted-foreground">
										ใช้มุมมองนี้เมื่อต้องจัดรายละเอียดของรายวิชาหรือกิจกรรมใดกิจกรรมหนึ่ง
									</p>
								</div>
								<p class="rounded-full bg-primary/10 px-3 py-1 text-sm font-medium text-primary">
									{items.length} รายการ
								</p>
							</div>
							<OfferingOverviewTable {items} {initialKind} />
						</section>
					{/if}
				</Tabs.Content>
			</Tabs.Root>

			<p class="text-xs text-muted-foreground">
				ข้อมูลทั้งหมดอ้างอิงปีการศึกษาและภาคเรียนที่เลือกบนแถบด้านบน
				การเปลี่ยนบริบทจะโหลดโครงสร้างและการเปิดสอนของภาคเรียนนั้นใหม่
			</p>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</div>
	{/if}
</PageShell>
