<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/state';
	import { invalidate, replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { untrack } from 'svelte';
	import {
		LEARNING_DELIVERY_PAGE_DEPENDENCY,
		type LearningDeliveryRefreshScope
	} from '$lib/academic/learning-delivery-page';
	import {
		buildSynchronizedActivityPreparationTarget,
		type SynchronizedActivityPreparationTarget
	} from '$lib/academic/synchronized-activity-delivery';
	import {
		getLearningDeliveryOverview,
		type AcademicTermChangeSet,
		type HomeroomDeliveryWorkspace as HomeroomWorkspace,
		type LearningDeliveryOverview,
		type LearningDeliveryPageView,
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
	const overviewRequest = new LatestRequest();
	let workspace = $state.raw<HomeroomWorkspace | null>(null);
	let overview = $state.raw<LearningDeliveryOverview | null>(null);
	let changeSets = $state.raw<AcademicTermChangeSet[]>([]);
	let selectedChangeSetId = $state('');
	let overviewLoading = $state(false);
	let errorMessage = $state('');
	let pageLoadError = $derived(data.pageView && !data.pageView.ok ? data.pageView.error : '');
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
	let activeChangeSet = $derived(
		changeSets.find((changeSet) => changeSet.id === selectedChangeSetId) ??
			changeSets.find((changeSet) => changeSet.status === 'draft') ??
			changeSets[0] ??
			null
	);
	let activeChangeSetLabel = $derived(
		activeChangeSet ? formatChangeSetOption(activeChangeSet) : 'เลือกชุดการเปลี่ยนแปลง'
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

	function formatChangeSetOption(changeSet: AcademicTermChangeSet): string {
		const status =
			changeSet.status === 'draft'
				? 'แบบร่าง'
				: changeSet.status === 'published'
					? 'เผยแพร่แล้ว'
					: 'ยกเลิกแล้ว';
		return `${formatDate(changeSet.effectiveFrom)} · ${status} · ${changeSet.reason}`;
	}

	function applyPageView(loaded: LearningDeliveryPageView) {
		workspace = loaded.workspace;
		changeSets = loaded.changeSets;
		overview = loaded.overview;
		const requestedId = page.url.searchParams.get('changeSetId')?.trim() ?? '';
		selectedChangeSetId =
			loaded.changeSets.find((item) => item.id === requestedId)?.id ??
			loaded.changeSets.find((item) => item.status === 'draft')?.id ??
			loaded.changeSets[0]?.id ??
			'';
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

	async function refreshDeliveryPage(refreshOverview = viewMode === 'offerings') {
		await invalidate(LEARNING_DELIVERY_PAGE_DEPENDENCY);
		if (refreshOverview && academicTermId) await loadOverview(academicTermId);
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
		void refreshDeliveryPage(true);
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
			await refreshDeliveryPage();
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
		await refreshDeliveryPage();
		if (pending?.kind !== 'activate' || !workspace || !offeringDialog) return;
		const target = buildSynchronizedActivityPreparationTarget(workspace, pending.catalogVersionId);
		if (!target) return;
		await offeringDialog.openCurriculumPreparation(target, created.targetTimetableVersionId);
	}

	function addChangeSet(created: AcademicTermChangeSet) {
		changeSets = [created, ...changeSets.filter((changeSet) => changeSet.id !== created.id)];
		selectedChangeSetId = created.id;
	}

	async function updateChangeSet(
		updated: AcademicTermChangeSet,
		refreshScope: LearningDeliveryRefreshScope = 'local'
	) {
		selectedChangeSetId = updated.id;
		changeSets = changeSets
			.map((changeSet) => (changeSet.id === updated.id ? updated : changeSet))
			.sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
		if (updated.items.length > 0 && !overview && academicTermId) {
			await loadOverview(academicTermId);
		}
		if (refreshScope === 'page') {
			await invalidate(LEARNING_DELIVERY_PAGE_DEPENDENCY);
		}
	}

	$effect(() => {
		const routeResult = data.pageView;
		overviewRequest.abort();
		errorMessage = '';
		if (routeResult?.ok) {
			untrack(() => applyPageView(routeResult.data));
		} else {
			workspace = null;
			overview = null;
			changeSets = [];
			selectedChangeSetId = '';
		}
		return () => {
			overviewRequest.abort();
		};
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
				onApplied={() => refreshDeliveryPage(true)}
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
	{:else if pageLoadError && !workspace}
		<PageState
			variant="error"
			title="โหลดพื้นที่จัดการการเปิดสอนไม่สำเร็จ"
			description={pageLoadError}
			actionLabel="ลองอีกครั้ง"
			onaction={() => invalidate(LEARNING_DELIVERY_PAGE_DEPENDENCY)}
		/>
	{:else}
		<div class="space-y-4">
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
						value={activeChangeSet?.id ?? ''}
						onValueChange={(value) => (selectedChangeSetId = value)}
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
			{#if activeChangeSet}
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
			<Tabs.Root value={viewMode} onValueChange={changeViewMode}>
				<Tabs.List class="grid w-full grid-cols-2 sm:w-[430px]">
					<Tabs.Trigger value="homerooms">มุมมองรายห้อง</Tabs.Trigger>
					<Tabs.Trigger value="offerings">มุมมองรายวิชา/กิจกรรม</Tabs.Trigger>
				</Tabs.List>
				<Tabs.Content value="homerooms" class="mt-4">
					{#if workspace}
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
