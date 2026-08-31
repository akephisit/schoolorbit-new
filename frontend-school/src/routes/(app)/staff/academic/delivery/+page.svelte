<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import {
		getHomeroomDeliveryWorkspace,
		getLearningDeliveryOverview,
		listAcademicTermChangeSets,
		type AcademicTermChangeSet,
		type HomeroomDeliveryWorkspace as HomeroomWorkspace,
		type LearningDeliveryOverview,
		type LearningOfferingOverviewItem
	} from '$lib/api/learning-delivery';
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

	const academicContext = getAcademicContextStore();
	const academicYearId = $derived($academicContext.selected.academicYearId);
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const workspaceRequest = new LatestRequest();
	const overviewRequest = new LatestRequest();
	let workspace = $state.raw<HomeroomWorkspace | null>(null);
	let overview = $state.raw<LearningDeliveryOverview | null>(null);
	let changeSets = $state.raw<AcademicTermChangeSet[]>([]);
	let selectedChangeSetId = $state('');
	let loading = $state(false);
	let overviewLoading = $state(false);
	let errorMessage = $state('');
	let viewMode = $state<'homerooms' | 'offerings'>('homerooms');
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

	async function loadWorkspace(yearId: string, termId: string) {
		const { revision, signal } = workspaceRequest.begin();
		loading = true;
		errorMessage = '';
		try {
			const timetableVersionId =
				page.url.searchParams.get('timetableVersionId')?.trim() || undefined;
			const homeroomResult = await getHomeroomDeliveryWorkspace(yearId, termId, {
				signal,
				timetableVersionId
			});
			const loadedChangeSets = await listAcademicTermChangeSets(termId, { signal });
			if (!workspaceRequest.isCurrent(revision)) return;
			workspace = homeroomResult;
			changeSets = loadedChangeSets;
			if (!loadedChangeSets.some((changeSet) => changeSet.id === selectedChangeSetId)) {
				const requestedChangeSetId = page.url.searchParams.get('changeSetId')?.trim() ?? '';
				selectedChangeSetId =
					loadedChangeSets.find((changeSet) => changeSet.id === requestedChangeSetId)?.id ??
					loadedChangeSets.find((changeSet) => changeSet.status === 'draft')?.id ??
					loadedChangeSets[0]?.id ??
					'';
			}
			if (
				(loadedChangeSets.some((changeSet) => changeSet.items.length > 0) ||
					(canManage && loadedChangeSets.some((changeSet) => changeSet.status === 'draft'))) &&
				!overview &&
				!overviewLoading
			)
				void loadOverview(termId);
		} catch (error) {
			if (isAbortError(error)) return;
			if (workspaceRequest.isCurrent(revision))
				errorMessage =
					error instanceof Error ? error.message : 'โหลดพื้นที่จัดการการเปิดสอนไม่สำเร็จ';
		} finally {
			if (workspaceRequest.isCurrent(revision)) loading = false;
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
		if (academicYearId && academicTermId) void loadWorkspace(academicYearId, academicTermId);
	}

	async function reloadAfterApply() {
		if (!academicYearId || !academicTermId) return;
		await loadWorkspace(academicYearId, academicTermId);
		if (viewMode === 'offerings' || overview) await loadOverview(academicTermId);
	}

	function addChangeSet(created: AcademicTermChangeSet) {
		changeSets = [created, ...changeSets.filter((changeSet) => changeSet.id !== created.id)];
		selectedChangeSetId = created.id;
		if (academicTermId && !overviewLoading) void loadOverview(academicTermId);
	}

	async function updateChangeSet(updated: AcademicTermChangeSet) {
		selectedChangeSetId = updated.id;
		changeSets = changeSets
			.map((changeSet) => (changeSet.id === updated.id ? updated : changeSet))
			.sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
		if (!academicYearId || !academicTermId) return;
		await loadOverview(academicTermId);
		await loadWorkspace(academicYearId, academicTermId);
	}

	onMount(() => {
		let loadedContext = '';
		const unsubscribe = academicContext.subscribe((state) => {
			const yearId = state.selected.academicYearId;
			const termId = state.selected.academicTermId;
			const contextKey = yearId && termId ? `${yearId}:${termId}` : '';
			if (yearId && termId && contextKey !== loadedContext) {
				loadedContext = contextKey;
				workspaceRequest.abort();
				overviewRequest.abort();
				workspace = null;
				overview = null;
				changeSets = [];
				selectedChangeSetId = '';
				errorMessage = '';
				void loadWorkspace(yearId, termId).then(() => {
					if (viewMode === 'offerings') void loadOverview(termId);
				});
			} else if (!contextKey) {
				loadedContext = '';
				workspace = null;
				overview = null;
				changeSets = [];
				selectedChangeSetId = '';
				loading = false;
				errorMessage = '';
				workspaceRequest.abort();
				overviewRequest.abort();
			}
		});
		return () => {
			unsubscribe();
			workspaceRequest.abort();
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
			<OfferingCreateDialog {academicTermId} onCreated={addCreated} onApplied={reloadAfterApply} />
			<AcademicChangeSetDialog {academicTermId} onCreated={addChangeSet} />
		{/if}
	{/snippet}

	{#if !academicYearId || !academicTermId}
		<AcademicPrerequisiteNotice prerequisite={missingTermPrerequisite} />
	{:else if loading && !workspace}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && !workspace}
		<PageState
			variant="error"
			title="โหลดพื้นที่จัดการการเปิดสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(academicYearId, academicTermId)}
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
						<HomeroomDeliveryWorkspace {workspace} />
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
