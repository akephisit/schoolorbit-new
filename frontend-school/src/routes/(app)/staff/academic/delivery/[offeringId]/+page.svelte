<script lang="ts">
	import { afterNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { ApiClientError } from '$lib/api/client';
	import {
		applyLearningGroupRoster,
		createLearningGroup,
		getLearningDeliveryManagementOptions,
		getLearningGroup,
		getLearningOffering,
		listLearningGroups,
		previewLearningGroupRoster,
		publishLearningGroupRoster,
		publishLearningOffering,
		replaceLearningGroupHomerooms,
		replaceLearningGroupTeachers,
		updateLearningGroup,
		type CreateLearningGroupRequest,
		type DeliveryManagementOptions,
		type LearningGroup,
		type LearningOffering,
		type ReplaceLearningGroupHomeroomsRequest,
		type ReplaceLearningGroupTeachersRequest,
		type RosterPreview,
		type UpdateLearningGroupRequest
	} from '$lib/api/learning-delivery';
	import {
		listTimetableVersions,
		type TimetableVersion,
		type TimetableVersionTarget
	} from '$lib/api/timetable';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import LearningGroupEditor from '$lib/components/learning-delivery/LearningGroupEditor.svelte';
	import LearningGroupList from '$lib/components/learning-delivery/LearningGroupList.svelte';
	import DatedRosterMemberships from '$lib/components/learning-delivery/DatedRosterMemberships.svelte';
	import RosterPreviewPanel from '$lib/components/learning-delivery/RosterPreviewPanel.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		ArrowLeft,
		ArrowRight,
		BookOpenCheck,
		ClipboardList,
		Pencil,
		Send,
		UsersRound
	} from 'lucide-svelte';

	const detailRequest = new LatestRequest();
	const groupRequest = new LatestRequest();
	const offeringId = $derived(page.params.offeringId ?? '');

	let offering = $state.raw<LearningOffering | null>(null);
	let groups = $state.raw<LearningGroup[]>([]);
	let selectedGroup = $state.raw<LearningGroup | null>(null);
	let timetableVersions = $state<TimetableVersion[]>([]);
	let selectedTimetableVersion = $state.raw<TimetableVersion | null>(null);
	let timetableVersionSelectValue = $state('');
	let managementOptions = $state.raw<DeliveryManagementOptions | null>(null);
	let rosterPreview = $state.raw<RosterPreview | null>(null);
	let loading = $state(true);
	let groupLoading = $state(false);
	let optionsLoading = $state(false);
	let rosterLoading = $state(false);
	let publishing = $state(false);
	let initialized = $state(false);
	let editorVisible = $state(false);
	let rosterVisible = $state(false);
	let rosterStale = $state(false);
	let errorMessage = $state('');
	let actionError = $state('');

	let canManage = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	let offeringIsReadOnly = $derived(
		offering?.status === 'cancelled' || offering?.status === 'closed'
	);
	let canMutateOffering = $derived(canManage && offering !== null && !offeringIsReadOnly);
	let courseSnapshot = $derived(offering?.snapshot.kind === 'course' ? offering.snapshot : null);
	let selectedTimetableTarget = $derived<TimetableVersionTarget | null>(
		selectedTimetableVersion?.targets.find(
			(target) => target.learningOfferingId === offering?.id
		) ?? null
	);

	function preferredTimetableVersion(loadedVersions: TimetableVersion[]): TimetableVersion | null {
		const requestedId = page.url.searchParams.get('timetableVersionId');
		return (
			loadedVersions.find((version) => version.id === requestedId) ??
			loadedVersions.find(
				(version) => version.status === 'published' && version.displayState === 'current'
			) ??
			loadedVersions.find(
				(version) => version.status === 'published' && version.displayState === 'upcoming'
			) ??
			loadedVersions.find((version) => version.status === 'draft') ??
			null
		);
	}

	function timetableVersionLabel(version: TimetableVersion): string {
		const status =
			version.status === 'draft'
				? 'แบบร่าง'
				: version.status === 'published'
					? 'เผยแพร่แล้ว'
					: 'ยกเลิกแล้ว';
		return `${status} · เริ่ม ${version.effectiveFrom}`;
	}

	function selectTimetableVersion(versionId: string): void {
		const version = timetableVersions.find((item) => item.id === versionId);
		if (!version) return;
		selectedTimetableVersion = version;
		timetableVersionSelectValue = version.id;
		const nextUrl = new URL(page.url);
		nextUrl.searchParams.set('timetableVersionId', version.id);
		window.history.replaceState(window.history.state, '', nextUrl);
	}

	function offeringKindLabel(kind: LearningOffering['kind']) {
		return kind === 'course' ? 'รายวิชา' : 'กิจกรรมพัฒนาผู้เรียน';
	}

	function offeringStatusLabel(status: LearningOffering['status']) {
		if (status === 'published') return 'เผยแพร่แล้ว';
		if (status === 'closed') return 'ปิดแล้ว';
		if (status === 'cancelled') return 'ยกเลิกแล้ว';
		return 'ฉบับร่าง';
	}

	function rosterStatusLabel(status: LearningGroup['rosterStatus']) {
		if (status === 'published') return 'เผยแพร่แล้ว';
		if (status === 'closed') return 'ปิดแล้ว';
		return 'ฉบับร่าง';
	}

	function updateGroupState(updated: LearningGroup) {
		groups = groups.map((group) => (group.id === updated.id ? updated : group));
		selectedGroup = updated;
	}

	function resetSelectedWorkspace() {
		editorVisible = false;
		rosterVisible = false;
		rosterPreview = null;
		rosterStale = false;
		actionError = '';
	}

	async function navigateToGroup(group: LearningGroup, replaceState = false) {
		selectedGroup = group;
		resetSelectedWorkspace();
		const versionQuery = selectedTimetableVersion
			? `&timetableVersionId=${encodeURIComponent(selectedTimetableVersion.id)}`
			: '';
		await goto(
			resolve(
				`/staff/academic/delivery/${offeringId}?groupId=${encodeURIComponent(group.id)}${versionQuery}`
			),
			{ replaceState, keepFocus: true, noScroll: true }
		);
	}

	async function loadDetail() {
		const { revision, signal } = detailRequest.begin();
		loading = true;
		errorMessage = '';
		try {
			const loadedOffering = await getLearningOffering(offeringId, { signal });
			const loadedTimetableVersions = await listTimetableVersions(loadedOffering.academicTermId, {
				signal
			});
			const loadedGroups = await listLearningGroups(offeringId, { signal });
			const requestedGroupId = page.url.searchParams.get('groupId');
			const targetGroup =
				loadedGroups.find((group) => group.id === requestedGroupId) ?? loadedGroups[0] ?? null;
			const loadedGroup = targetGroup ? await getLearningGroup(targetGroup.id, { signal }) : null;
			if (!detailRequest.isCurrent(revision)) return;
			offering = loadedOffering;
			timetableVersions = loadedTimetableVersions;
			selectedTimetableVersion = preferredTimetableVersion(loadedTimetableVersions);
			timetableVersionSelectValue = selectedTimetableVersion?.id ?? '';
			groups = loadedGroups.map((group) => (group.id === loadedGroup?.id ? loadedGroup : group));
			selectedGroup = loadedGroup;
			initialized = true;
			if (loadedGroup && requestedGroupId !== loadedGroup.id) {
				await navigateToGroup(loadedGroup, true);
			}
		} catch (error) {
			if (isAbortError(error)) return;
			if (detailRequest.isCurrent(revision)) {
				errorMessage =
					error instanceof Error ? error.message : 'โหลดรายละเอียดรายการเปิดสอนไม่สำเร็จ';
			}
		} finally {
			if (detailRequest.isCurrent(revision)) loading = false;
		}
	}

	async function loadSelectedGroup(groupId: string) {
		const known = groups.some((group) => group.id === groupId);
		if (!known) return;
		const { revision, signal } = groupRequest.begin();
		groupLoading = true;
		actionError = '';
		try {
			const loaded = await getLearningGroup(groupId, { signal });
			if (!groupRequest.isCurrent(revision) || loaded.learningOfferingId !== offeringId) return;
			updateGroupState(loaded);
			resetSelectedWorkspace();
		} catch (error) {
			if (isAbortError(error)) return;
			if (groupRequest.isCurrent(revision)) {
				actionError = error instanceof Error ? error.message : 'โหลดกลุ่มเรียนไม่สำเร็จ';
			}
		} finally {
			if (groupRequest.isCurrent(revision)) groupLoading = false;
		}
	}

	async function requestManagementOptions() {
		if (!canMutateOffering || !offering) return null;
		if (managementOptions) return managementOptions;
		if (optionsLoading) return null;
		optionsLoading = true;
		actionError = '';
		try {
			managementOptions = await getLearningDeliveryManagementOptions(offering.academicTermId);
			return managementOptions;
		} catch (error) {
			actionError =
				error instanceof Error ? error.message : 'โหลดตัวเลือกสำหรับจัดการกลุ่มไม่สำเร็จ';
			throw error;
		} finally {
			optionsLoading = false;
		}
	}

	async function showEditor() {
		try {
			const options = await requestManagementOptions();
			if (options) editorVisible = true;
		} catch {
			// The actionable error remains next to the selected group.
		}
	}

	async function showRoster() {
		try {
			const options = await requestManagementOptions();
			if (!options) return;
			rosterVisible = true;
			if (!rosterPreview) await refreshRoster();
		} catch {
			// The actionable error remains next to the selected group.
		}
	}

	async function createGroup(request: CreateLearningGroupRequest) {
		if (!canMutateOffering) return;
		const created = await createLearningGroup(offeringId, request);
		groups = [...groups, created].sort((left, right) =>
			left.code.localeCompare(right.code, 'th-TH', { numeric: true })
		);
		await navigateToGroup(created);
	}

	async function saveGroup(request: UpdateLearningGroupRequest) {
		if (!canMutateOffering || !selectedGroup) return;
		const updated = await updateLearningGroup(selectedGroup.id, request);
		updateGroupState(updated);
	}

	async function replaceTeachers(request: ReplaceLearningGroupTeachersRequest) {
		if (!canMutateOffering || !selectedGroup) return;
		if (selectedGroup.teachersLocked) {
			actionError = 'เผยแพร่กลุ่มเรียนแล้ว ไม่สามารถเปลี่ยนครูผู้สอนได้';
			return;
		}
		const updated = await replaceLearningGroupTeachers(selectedGroup.id, request);
		updateGroupState(updated);
	}

	async function replaceHomerooms(request: ReplaceLearningGroupHomeroomsRequest) {
		if (!canMutateOffering || !selectedGroup) return;
		const updated = await replaceLearningGroupHomerooms(selectedGroup.id, request);
		updateGroupState(updated);
		rosterStale = rosterPreview !== null;
	}

	async function refreshRoster() {
		if (!canMutateOffering || !selectedGroup) return;
		const groupId = selectedGroup.id;
		rosterLoading = true;
		try {
			const refreshedGroup = await getLearningGroup(groupId);
			const refreshedPreview = await previewLearningGroupRoster(groupId);
			if (selectedGroup?.id !== groupId) return;
			updateGroupState(refreshedGroup);
			rosterPreview = refreshedPreview;
			rosterStale = false;
		} finally {
			rosterLoading = false;
		}
	}

	async function applyRoster(sourceHash: string) {
		if (!canMutateOffering || !selectedGroup) return;
		rosterLoading = true;
		try {
			const updated = await applyLearningGroupRoster(selectedGroup.id, {
				sourceHash,
				overrides: [],
				rowVersion: selectedGroup.rowVersion
			});
			updateGroupState(updated);
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) rosterStale = true;
			throw error;
		} finally {
			rosterLoading = false;
		}
	}

	async function publishRoster() {
		if (!canMutateOffering || !selectedGroup) return;
		rosterLoading = true;
		try {
			const updated = await publishLearningGroupRoster(selectedGroup.id, {
				rowVersion: selectedGroup.rowVersion,
				idempotencyKey: crypto.randomUUID()
			});
			updateGroupState(updated);
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) rosterStale = true;
			throw error;
		} finally {
			rosterLoading = false;
		}
	}

	async function refreshSelectedGroupAfterMembership() {
		if (!selectedGroup) return;
		const refreshed = await getLearningGroup(selectedGroup.id);
		updateGroupState(refreshed);
	}

	async function publishOfferingNow() {
		if (!canMutateOffering || !offering || offering.status !== 'draft') return;
		publishing = true;
		actionError = '';
		try {
			offering = await publishLearningOffering(offering.id, {
				rowVersion: offering.rowVersion,
				idempotencyKey: crypto.randomUUID()
			});
		} catch (error) {
			actionError = error instanceof Error ? error.message : 'เผยแพร่รายการเปิดสอนไม่สำเร็จ';
		} finally {
			publishing = false;
		}
	}

	afterNavigate(({ to }) => {
		if (!initialized) return;
		const requestedGroupId = to?.url.searchParams.get('groupId') ?? '';
		if (requestedGroupId && requestedGroupId !== selectedGroup?.id) {
			void loadSelectedGroup(requestedGroupId);
		}
	});

	onMount(() => {
		void loadDetail();
		return () => {
			detailRequest.abort();
			groupRequest.abort();
		};
	});
</script>

<PageShell
	title={offering?.nameSnapshot ?? 'รายละเอียดรายการเปิดสอน'}
	description="จัดกลุ่มเรียน ครู ห้อง และรายชื่อนักเรียนของรายการนี้ โดยข้อมูลทั้งหมดอยู่ในภาคเรียนเดียวกัน"
>
	{#snippet actions()}
		<div class="flex flex-wrap gap-2">
			<Button href="/staff/academic/delivery" variant="outline">
				<ArrowLeft class="size-4" /> กลับภาพรวม
			</Button>
			{#if offering && canManage && offering.status === 'draft'}
				<Button onclick={publishOfferingNow} disabled={publishing || groups.length === 0}>
					<Send class="size-4" />
					{publishing ? 'กำลังเผยแพร่' : 'เผยแพร่รายการเปิดสอน'}
				</Button>
			{/if}
		</div>
	{/snippet}

	{#if loading}
		<PageSkeleton variant="cards" rows={5} />
	{:else if errorMessage || !offering}
		<PageState
			variant="error"
			title="โหลดรายละเอียดรายการเปิดสอนไม่สำเร็จ"
			description={errorMessage || 'ไม่พบรายการเปิดสอน'}
			actionLabel="ลองอีกครั้ง"
			onaction={loadDetail}
		/>
	{:else}
		<div class="space-y-5">
			<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
				<div class="grid gap-5 p-5 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center">
					<div class="flex min-w-0 items-start gap-4">
						<div class="rounded-2xl bg-primary/10 p-3 text-primary">
							<BookOpenCheck class="size-6" />
						</div>
						<div class="min-w-0">
							<div class="flex flex-wrap items-center gap-2">
								<Badge variant={offering.kind === 'course' ? 'default' : 'secondary'}>
									{offeringKindLabel(offering.kind)}
								</Badge>
								<Badge variant="outline">{offeringStatusLabel(offering.status)}</Badge>
							</div>
							<h2 class="mt-3 text-xl font-semibold tracking-tight">{offering.nameSnapshot}</h2>
							<p class="mt-1 font-mono text-sm text-muted-foreground">{offering.codeSnapshot}</p>
						</div>
					</div>
					<div class="grid grid-cols-3 gap-2 text-center">
						<div class="rounded-xl bg-muted/60 px-4 py-3">
							<p class="text-lg font-semibold">{groups.length}</p>
							<p class="text-xs text-muted-foreground">กลุ่มเรียน</p>
						</div>
						<div class="rounded-xl bg-muted/60 px-4 py-3">
							<p class="text-lg font-semibold">
								{groups.reduce((sum, group) => sum + group.teacherAssignments.length, 0)}
							</p>
							<p class="text-xs text-muted-foreground">ครูที่มอบหมาย</p>
						</div>
						<div class="rounded-xl bg-muted/60 px-4 py-3">
							<p class="text-lg font-semibold">
								{groups.filter((group) => group.rosterStatus === 'published').length}
							</p>
							<p class="text-xs text-muted-foreground">รายชื่อพร้อมใช้</p>
						</div>
					</div>
				</div>
				<div class="border-t border-primary/15 bg-primary/[0.045] px-5 py-4">
					<div
						class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto_minmax(0,1.35fr)] sm:items-center"
					>
						<div class="rounded-xl border border-primary/15 bg-background/80 px-4 py-3">
							<p class="text-xs font-medium text-muted-foreground">
								{courseSnapshot ? 'ตามหลักสูตร' : 'ตามทะเบียนกิจกรรม'}
							</p>
							{#if courseSnapshot}
								<p class="mt-1 font-mono text-lg font-semibold tabular-nums text-foreground">
									{courseSnapshot.standardPeriodsPerWeek} คาบ/สัปดาห์
								</p>
								<p class="mt-0.5 text-xs text-muted-foreground">ค่ามาตรฐานของรหัสวิชา</p>
							{:else}
								<p class="mt-1 text-sm font-medium text-foreground">กิจกรรมไม่มีค่าคาบมาตรฐาน</p>
								<p class="mt-0.5 text-xs text-muted-foreground">ใช้เป้าหมายของรุ่นตารางสอนโดยตรง</p>
							{/if}
						</div>
						<div class="hidden text-primary sm:block" aria-hidden="true">
							<ArrowRight class="size-5" />
						</div>
						<div class="rounded-xl border border-primary/25 bg-background px-4 py-3 shadow-sm">
							<p class="text-xs font-medium text-muted-foreground">จัดจริงภาคเรียนนี้</p>
							{#if timetableVersions.length > 0}
								<Select.Root
									type="single"
									bind:value={timetableVersionSelectValue}
									onValueChange={selectTimetableVersion}
								>
									<Select.Trigger class="mt-1.5 w-full" aria-label="เลือกรุ่นตารางสอน">
										{selectedTimetableVersion
											? timetableVersionLabel(selectedTimetableVersion)
											: 'เลือกรุ่นตารางสอน'}
									</Select.Trigger>
									<Select.Content>
										{#each timetableVersions as version (version.id)}
											<Select.Item value={version.id}>{timetableVersionLabel(version)}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							{/if}
							<p class="mt-1 font-mono text-lg font-semibold tabular-nums text-primary">
								{selectedTimetableTarget
									? `${selectedTimetableTarget.weeklyPeriodTarget} คาบ/สัปดาห์`
									: 'ยังไม่กำหนด'}
							</p>
							<p class="mt-0.5 text-xs text-muted-foreground">
								{selectedTimetableVersion
									? `รุ่นตารางเริ่มใช้ ${selectedTimetableVersion.effectiveFrom} · ${selectedTimetableVersion.status === 'draft' ? 'แบบร่าง' : 'เผยแพร่แล้ว'}`
									: 'ยังไม่มีรุ่นตารางสอนที่ใช้อ้างอิง'}
							</p>
						</div>
					</div>
				</div>
			</section>

			{#if offeringIsReadOnly}
				<section class="rounded-xl border bg-muted/30 px-4 py-3 text-sm">
					<p class="font-medium">
						{offering.status === 'cancelled'
							? 'รายการเปิดสอนนี้ยกเลิกแล้ว'
							: 'รายการเปิดสอนนี้ปิดแล้ว'}
					</p>
					<p class="mt-1 text-xs text-muted-foreground">
						เปิดดูข้อมูลและประวัติได้ แต่ไม่สามารถเพิ่มหรือแก้ไขกลุ่ม ครู ห้อง และรายชื่อนักเรียน
					</p>
				</section>
			{/if}

			<LearningGroupList
				{groups}
				selectedGroupId={selectedGroup?.id}
				canManage={canMutateOffering}
				onSelect={navigateToGroup}
				onRequestManagementOptions={requestManagementOptions}
				onCreate={createGroup}
			/>

			{#if groupLoading}
				<PageSkeleton variant="cards" rows={3} />
			{:else if selectedGroup}
				<section class="rounded-2xl border bg-card p-4 shadow-sm">
					<div class="flex flex-wrap items-center justify-between gap-4">
						<div class="flex min-w-0 items-center gap-3">
							<div class="rounded-xl bg-primary/10 p-2.5 text-primary">
								<UsersRound class="size-5" />
							</div>
							<div class="min-w-0">
								<div class="flex flex-wrap items-center gap-2">
									<h2 class="truncate font-semibold">{selectedGroup.name}</h2>
									{#if selectedGroup.teachersLocked}
										<Badge variant="outline">ครูผู้สอนถูกล็อกแล้ว</Badge>
									{/if}
								</div>
								<p class="text-sm text-muted-foreground">
									{selectedGroup.code} · ครู {selectedGroup.teacherAssignments.length} คน · ห้องต้นทาง
									{selectedGroup.homeroomIds.length} ห้อง · รายชื่อ {rosterStatusLabel(
										selectedGroup.rosterStatus
									)}
								</p>
							</div>
						</div>
						{#if canMutateOffering}
							<div class="flex flex-wrap gap-2">
								<Button variant="outline" onclick={showEditor} disabled={optionsLoading}>
									<Pencil class="size-4" />
									{optionsLoading ? 'กำลังโหลด' : 'จัดการกลุ่ม'}
								</Button>
								{#if selectedGroup.rosterStatus !== 'published'}
									<Button
										onclick={showRoster}
										disabled={optionsLoading || selectedGroup.homeroomIds.length === 0}
									>
										<ClipboardList class="size-4" /> ตรวจรายชื่อนักเรียน
									</Button>
								{/if}
							</div>
						{/if}
					</div>
					{#if selectedGroup.homeroomIds.length === 0}
						<p
							class="mt-3 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-900"
						>
							กำหนดห้องต้นทางก่อน จึงจะสร้างตัวอย่างรายชื่อนักเรียนได้
						</p>
					{/if}
				</section>

				{#if editorVisible && managementOptions}
					{#key `${selectedGroup.id}:${selectedGroup.rowVersion}`}
						<LearningGroupEditor
							group={selectedGroup}
							{managementOptions}
							canManage={canMutateOffering}
							onSaveGroup={saveGroup}
							onReplaceTeachers={replaceTeachers}
							onReplaceHomerooms={replaceHomerooms}
						/>
					{/key}
				{/if}

				{#if selectedGroup.rosterStatus === 'published'}
					{#key `${selectedGroup.id}:${selectedGroup.rowVersion}`}
						<DatedRosterMemberships
							group={selectedGroup}
							canManage={canMutateOffering}
							onGroupChanged={refreshSelectedGroupAfterMembership}
						/>
					{/key}
				{:else if rosterVisible}
					<RosterPreviewPanel
						group={selectedGroup}
						preview={rosterPreview}
						loading={rosterLoading}
						stale={rosterStale}
						canManage={canMutateOffering}
						onRefresh={refreshRoster}
						onApply={applyRoster}
						onPublish={publishRoster}
					/>
				{/if}
			{:else}
				<section
					class="rounded-2xl border border-dashed p-10 text-center text-sm text-muted-foreground"
				>
					เพิ่มหรือเลือกกลุ่มเรียนเพื่อจัดครู ห้อง และรายชื่อนักเรียน
				</section>
			{/if}

			{#if actionError}
				<p
					role="alert"
					class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
				>
					{actionError}
				</p>
			{/if}
		</div>
	{/if}
</PageShell>
