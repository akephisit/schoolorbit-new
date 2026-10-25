<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { onDestroy, untrack } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		createCatalogSubject,
		createSubjectVersion,
		deleteCatalogSubject,
		listSubjectVersions,
		publishSubjectVersion,
		updateCatalogSubject,
		type CatalogDisplayState,
		type CatalogSubjectOverview,
		type CatalogSubjectOverviewItem,
		type SubjectVersion
	} from '#lib/api/academic-core.js';
	import { subjectVersionSource } from '#lib/academic-core/catalog-version-draft.js';
	import { CATALOG_SUBJECT_OVERVIEW_DEPENDENCY } from '#lib/academic-core/catalog-route.js';
	import {
		CATALOG_DISPLAY_STATE_OPTIONS,
		SUBJECT_TYPE_OPTIONS,
		displayStateClass,
		displayStateLabel,
		formatEffectiveRange,
		gradeLevelSummary,
		matchesCatalogSearch,
		optionLabel
	} from '#lib/academic-core/catalog-presentation.js';
	import CatalogDeleteDialog from '#lib/components/academic-core/CatalogDeleteDialog.svelte';
	import CatalogVersionHistory, {
		type CatalogVersionDraft
	} from '#lib/components/academic-core/CatalogVersionHistory.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Sheet from '#lib/components/ui/sheet/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { ArrowUpRight, BookOpen, Plus, Save, Search, SlidersHorizontal } from '@lucide/svelte';
	import type { PageProps } from './$types';

	const allValue = 'all';
	const collator = new Intl.Collator('th-TH', { numeric: true, sensitivity: 'base' });
	const subjectHistoryCache = new SvelteMap<string, SubjectVersion[]>();

	let { data }: PageProps = $props();
	let overview = $state.raw<CatalogSubjectOverview | null>(null);
	let selected = $state<CatalogSubjectOverviewItem | null>(null);
	let versions = $state.raw<SubjectVersion[]>([]);
	let sheetOpen = $state(false);
	let historyLoading = $state(false);
	let historyError = $state('');
	let loading = $state(true);
	let errorMessage = $state('');
	let mutationError = $state('');
	let createRevision = $state(0);
	let pendingCreateRevision = $state<number | null>(null);
	let creating = $derived(pendingCreateRevision === createRevision);
	let createOpen = $state(false);
	let filtersOpen = $state(false);
	let pageNumber = $state(1);
	const pageSize = 25;
	let code = $state('');
	let subjectGroupValue = $state('');
	let search = $state('');
	let typeFilter = $state(allValue);
	let subjectGroupFilter = $state(allValue);
	let editingSubjectGroupValue = $state('');
	let selectionRevision = $state(0);
	let pendingMetadataRevision = $state<number | null>(null);
	let subjectMetadataSaving = $derived(pendingMetadataRevision === selectionRevision);
	let subjectMetadataError = $state('');
	let gradeFilter = $state(allValue);
	let stateFilter = $state<CatalogDisplayState | typeof allValue>(allValue);
	const historyRequests = new LatestRequest();
	const deletedSubjectIds = new SvelteSet<string>();
	let pendingOpenSubjectId = '';
	let pendingOpenSelectionRevision = 0;
	let pendingOpenCreateRevision = 0;
	let disposed = false;
	onDestroy(() => {
		disposed = true;
		historyRequests.abort();
	});

	let subjectItems = $derived(overview?.items ?? []);
	let gradeLevelOptions = $derived(overview?.gradeLevelOptions ?? []);
	let subjectGroupOptions = $derived(overview?.subjectGroupOptions ?? []);
	let manageableSubjectGroupOptions = $derived(
		subjectGroupOptions.filter((option) => option.canManage)
	);
	let canCreate = $derived(manageableSubjectGroupOptions.length > 0);
	let selectedSubjectGroup = $derived(
		manageableSubjectGroupOptions.find((option) => option.subjectGroupId === subjectGroupValue)
	);
	let filteredSubjects = $derived.by(() =>
		subjectItems
			.filter((item) => {
				const version = item.displayVersion;
				const subjectGroup = subjectGroupOptions.find(
					(option) => option.subjectGroupId === item.subject.subjectGroupId
				);
				return (
					matchesCatalogSearch(
						search,
						item.subject.code,
						version?.nameTh,
						version?.nameEn,
						subjectGroup?.name
					) &&
					(typeFilter === allValue || version?.subjectType === typeFilter) &&
					(subjectGroupFilter === allValue || item.subject.subjectGroupId === subjectGroupFilter) &&
					(gradeFilter === allValue ||
						item.gradeLevels.some((level) => level.id === gradeFilter)) &&
					(stateFilter === allValue || item.displayState === stateFilter)
				);
			})
			.sort((left, right) => {
				const codeOrder = collator.compare(left.subject.code, right.subject.code);
				if (codeOrder !== 0) return codeOrder;
				return collator.compare(
					left.displayVersion?.nameTh ?? '',
					right.displayVersion?.nameTh ?? ''
				);
			})
	);

	let hasFilters = $derived(
		search.trim() !== '' ||
			[typeFilter, subjectGroupFilter, gradeFilter, stateFilter].some((value) => value !== allValue)
	);
	let activeFilterLabels = $derived(
		[
			typeFilter === allValue ? '' : optionLabel(SUBJECT_TYPE_OPTIONS, typeFilter),
			subjectGroupFilter === allValue
				? ''
				: (subjectGroupOptions.find((option) => option.subjectGroupId === subjectGroupFilter)
						?.name ?? ''),
			gradeFilter === allValue
				? ''
				: (gradeLevelOptions.find((level) => level.id === gradeFilter)?.name ?? ''),
			stateFilter === allValue ? '' : displayStateLabel(stateFilter)
		].filter(Boolean)
	);
	let pageCount = $derived(Math.max(1, Math.ceil(filteredSubjects.length / pageSize)));
	let visibleSubjects = $derived(
		filteredSubjects.slice((pageNumber - 1) * pageSize, pageNumber * pageSize)
	);

	function clearFilters() {
		search = '';
		typeFilter = allValue;
		subjectGroupFilter = allValue;
		gradeFilter = allValue;
		stateFilter = allValue;
		pageNumber = 1;
	}

	function selectState(value: string) {
		const option = CATALOG_DISPLAY_STATE_OPTIONS.find((item) => item.value === value);
		if (value === allValue || option) {
			stateFilter = option?.value ?? allValue;
			pageNumber = 1;
		}
	}

	function subjectGroupName(id: string): string {
		return (
			subjectGroupOptions.find((option) => option.subjectGroupId === id)?.name ?? 'ไม่พบกลุ่มสาระ'
		);
	}

	function loadOverview() {
		return invalidate(CATALOG_SUBJECT_OVERVIEW_DEPENDENCY);
	}

	$effect.pre(() => {
		const routeResult = data.overview;
		let current = true;
		untrack(() => {
			loading = Boolean(routeResult);
			errorMessage = '';
		});
		if (routeResult) {
			void routeResult.then((result) => {
				if (!current) return;
				untrack(() => {
					if (result.ok) {
						const latest = {
							...result.data,
							items: result.data.items.filter((item) => !deletedSubjectIds.has(item.subject.id))
						};
						const selectedId = selected?.subject.id;
						overview = latest;
						pageNumber = 1;
						if (
							!latest.subjectGroupOptions.some(
								(option) => option.canManage && option.subjectGroupId === subjectGroupValue
							)
						) {
							subjectGroupValue =
								latest.subjectGroupOptions.find((option) => option.canManage)?.subjectGroupId ?? '';
						}
						if (selectedId) {
							selected = latest.items.find((item) => item.subject.id === selectedId) ?? null;
						}
						if (
							pendingOpenSubjectId &&
							(pendingOpenSelectionRevision !== selectionRevision ||
								pendingOpenCreateRevision !== createRevision)
						)
							pendingOpenSubjectId = '';
						if (pendingOpenSubjectId) {
							const item = latest.items.find((row) => row.subject.id === pendingOpenSubjectId);
							if (item) {
								pendingOpenSubjectId = '';
								void openSubject(item);
							}
						}
					} else errorMessage = result.error;
					loading = false;
				});
			});
		}
		return () => {
			current = false;
		};
	});

	async function openSubject(item: CatalogSubjectOverviewItem) {
		selectionRevision += 1;
		const request = historyRequests.begin();
		selected = item;
		editingSubjectGroupValue = item.subject.subjectGroupId;
		subjectMetadataError = '';
		sheetOpen = true;
		historyError = '';
		const cached = subjectHistoryCache.get(item.subject.id);
		if (cached) {
			versions = cached;
			historyLoading = false;
			return;
		}

		versions = [];
		historyLoading = true;
		try {
			const loaded = await listSubjectVersions(item.subject.id, { signal: request.signal });
			if (!historyRequests.isCurrent(request.revision)) return;
			subjectHistoryCache.set(item.subject.id, loaded);
			versions = loaded;
		} catch (error) {
			if (historyRequests.isCurrent(request.revision) && !isAbortError(error)) {
				historyError = error instanceof Error ? error.message : 'โหลดประวัติรายวิชาไม่สำเร็จ';
			}
		} finally {
			if (historyRequests.isCurrent(request.revision)) historyLoading = false;
		}
	}

	async function reloadSelectedHistory(refreshOverview = true) {
		if (!selected) return;
		const request = historyRequests.begin();
		const subjectId = selected.subject.id;
		subjectHistoryCache.delete(subjectId);
		historyLoading = true;
		historyError = '';
		try {
			const loaded = await listSubjectVersions(subjectId, { signal: request.signal });
			if (!historyRequests.isCurrent(request.revision)) return;
			subjectHistoryCache.set(subjectId, loaded);
			versions = loaded;
			if (refreshOverview) await loadOverview();
		} catch (error) {
			if (historyRequests.isCurrent(request.revision) && !isAbortError(error)) {
				historyError = error instanceof Error ? error.message : 'โหลดประวัติรายวิชาไม่สำเร็จ';
			}
		} finally {
			if (historyRequests.isCurrent(request.revision)) historyLoading = false;
		}
	}

	function setSheetOpen(open: boolean) {
		sheetOpen = open;
		if (!open) {
			historyRequests.abort();
			selectionRevision += 1;
		}
	}

	async function deleteSubject() {
		if (!selected?.canDelete) return;
		const target = selected.subject;
		await deleteCatalogSubject(target.id, { rowVersion: target.rowVersion });
		if (disposed) return;
		deletedSubjectIds.add(target.id);
		subjectHistoryCache.delete(target.id);
		if (pendingOpenSubjectId === target.id) pendingOpenSubjectId = '';
		if (overview) {
			overview = {
				...overview,
				items: overview.items.filter((item) => item.subject.id !== target.id)
			};
			pageNumber = Math.min(pageNumber, Math.max(1, Math.ceil(filteredSubjects.length / pageSize)));
		}
		if (selected?.subject.id === target.id) {
			setSheetOpen(false);
			selected = null;
			versions = [];
			historyLoading = false;
			historyError = '';
		}
	}

	async function reconcileSubjectMutation(subjectId: string) {
		if (disposed) return;
		subjectHistoryCache.delete(subjectId);
		if (sheetOpen && selected?.subject.id === subjectId) await reloadSelectedHistory();
		else await loadOverview();
	}

	function openCreateSubject() {
		createRevision += 1;
		code = '';
		mutationError = '';
		createOpen = true;
	}

	function setCreateOpen(open: boolean) {
		createOpen = open;
		if (!open) createRevision += 1;
	}

	async function addSubject(event: SubmitEvent) {
		event.preventDefault();
		if (creating || !canCreate) return;
		if (!selectedSubjectGroup) {
			mutationError = 'กรุณาเลือกกลุ่มสาระของรายวิชา';
			return;
		}
		const draftRevision = createRevision;
		pendingCreateRevision = draftRevision;
		mutationError = '';
		try {
			const created = await createCatalogSubject({
				code,
				subjectGroupId: selectedSubjectGroup.subjectGroupId
			});
			if (disposed) return;
			if (createOpen && createRevision === draftRevision) {
				code = '';
				createOpen = false;
				pendingOpenSubjectId = created.id;
				pendingOpenSelectionRevision = selectionRevision;
				pendingOpenCreateRevision = createRevision;
			}
			await loadOverview();
		} catch (error) {
			if (!disposed && createOpen && createRevision === draftRevision)
				mutationError = error instanceof Error ? error.message : 'เพิ่มรหัสรายวิชาไม่สำเร็จ';
		} finally {
			if (pendingCreateRevision === draftRevision) pendingCreateRevision = null;
		}
	}

	async function addVersion(draft: CatalogVersionDraft) {
		if (!selected?.canManage) return;
		const subjectId = selected.subject.id;
		await createSubjectVersion(subjectId, {
			nameTh: draft.name,
			nameEn: draft.secondaryName || null,
			credit: draft.exactValue,
			description: draft.description ?? null,
			effectiveFrom: draft.effectiveFrom,
			effectiveUntil: draft.effectiveUntil || null,
			gradeLevelIds: draft.gradeLevelIds,
			hoursPerSemester: Number.parseInt(draft.totalValue, 10),
			periodsPerWeek: Number.parseInt(draft.standardPeriodsPerWeek, 10),
			subjectType: draft.classification,
			termCode: draft.termCode ?? null
		});
		await reconcileSubjectMutation(subjectId);
	}

	async function saveSubjectGroup() {
		if (!selected?.canManage || !editingSubjectGroupValue || subjectMetadataSaving) return;
		const target = selected.subject;
		const targetGroupId = editingSubjectGroupValue;
		const draftRevision = selectionRevision;
		pendingMetadataRevision = draftRevision;
		subjectMetadataError = '';
		try {
			await updateCatalogSubject(target.id, {
				code: target.code,
				subjectGroupId: targetGroupId,
				archived: target.archivedAt !== null,
				rowVersion: target.rowVersion
			});
			if (!disposed) await loadOverview();
		} catch (error) {
			if (
				!disposed &&
				sheetOpen &&
				selectionRevision === draftRevision &&
				selected?.subject.id === target.id
			)
				subjectMetadataError = error instanceof Error ? error.message : 'เปลี่ยนกลุ่มสาระไม่สำเร็จ';
		} finally {
			if (pendingMetadataRevision === draftRevision) pendingMetadataRevision = null;
		}
	}

	async function publish(id: string, rowVersion: number) {
		if (!selected?.canManage) return;
		const subjectId = selected.subject.id;
		await publishSubjectVersion(id, { rowVersion });
		await reconcileSubjectMutation(subjectId);
	}

	function gradeLevelsFor(version: SubjectVersion) {
		return gradeLevelOptions.filter((level) => version.gradeLevelIds.includes(level.id));
	}
</script>

<PageShell
	title="ทะเบียนรายวิชา"
	description="รหัส ชื่อ ประเภท ระดับชั้น และภาระการเรียนของรายวิชา"
>
	{#snippet actions()}
		{#if canCreate}<Button onclick={openCreateSubject}
				><Plus class="size-4" aria-hidden="true" />เพิ่มรายวิชา</Button
			>{/if}
	{/snippet}
	{#if loading && !overview}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && !overview}
		<PageState
			variant="error"
			title="โหลดทะเบียนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadOverview}
		/>
	{:else}
		<div
			class="relative min-w-0 space-y-4"
			aria-label="ทะเบียนรายวิชา"
			aria-busy={loading}
			data-testid="catalog-subjects-ready"
		>
			<section
				class="space-y-3 rounded-xl border bg-card p-3 sm:p-4"
				aria-label="ค้นหาและกรองรายวิชา"
			>
				<div class="flex items-center gap-3">
					<div class="relative min-w-0 flex-1">
						<Search
							class="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
							aria-hidden="true"
						/>
						<Label class="sr-only" for="subject-search">ค้นหารายวิชา</Label>
						<Input
							id="subject-search"
							class="ps-9"
							bind:value={
								() => search,
								(value) => {
									search = value;
									pageNumber = 1;
								}
							}
							placeholder="รหัส ชื่อ หรือกลุ่มสาระ"
						/>
					</div>
					<Button
						variant="outline"
						size="sm"
						aria-expanded={filtersOpen}
						aria-controls="subject-filters"
						onclick={() => (filtersOpen = !filtersOpen)}
						><SlidersHorizontal
							class="size-4"
							aria-hidden="true"
						/>ตัวกรอง{#if activeFilterLabels.length}<Badge variant="secondary"
								>{activeFilterLabels.length}</Badge
							>{/if}</Button
					>
					{#if hasFilters}<Button variant="ghost" size="sm" onclick={clearFilters}
							>ล้างตัวกรอง</Button
						>{/if}
				</div>
				{#if filtersOpen}
					<div id="subject-filters" class="grid gap-3 border-t pt-3 sm:grid-cols-2 xl:grid-cols-4">
						<div class="space-y-1.5">
							<Label for="subject-type-filter">ประเภท</Label>
							<Select.Root
								type="single"
								bind:value={
									() => typeFilter,
									(value) => {
										typeFilter = value;
										pageNumber = 1;
									}
								}
							>
								<Select.Trigger
									id="subject-type-filter"
									aria-label="กรองประเภทรายวิชา"
									class="w-full"
									>{typeFilter === allValue
										? 'ทุกประเภท'
										: optionLabel(SUBJECT_TYPE_OPTIONS, typeFilter)}</Select.Trigger
								>
								<Select.Content
									><Select.Item value={allValue}>ทุกประเภท</Select.Item
									>{#each SUBJECT_TYPE_OPTIONS as option (option.value)}<Select.Item
											value={option.value}>{option.label}</Select.Item
										>{/each}</Select.Content
								>
							</Select.Root>
						</div>
						<div class="space-y-1.5">
							<Label for="subject-group-filter">กลุ่มสาระ</Label>
							<Select.Root
								type="single"
								bind:value={
									() => subjectGroupFilter,
									(value) => {
										subjectGroupFilter = value;
										pageNumber = 1;
									}
								}
							>
								<Select.Trigger id="subject-group-filter" aria-label="กรองกลุ่มสาระ" class="w-full"
									>{subjectGroupFilter === allValue
										? 'ทุกกลุ่มสาระ'
										: subjectGroupName(subjectGroupFilter)}</Select.Trigger
								>
								<Select.Content
									><Select.Item value={allValue}>ทุกกลุ่มสาระ</Select.Item
									>{#each subjectGroupOptions as option (option.subjectGroupId)}<Select.Item
											value={option.subjectGroupId}>{option.name}</Select.Item
										>{/each}</Select.Content
								>
							</Select.Root>
						</div>
						<div class="space-y-1.5">
							<Label for="subject-grade-filter">ระดับชั้น</Label>
							<Select.Root
								type="single"
								bind:value={
									() => gradeFilter,
									(value) => {
										gradeFilter = value;
										pageNumber = 1;
									}
								}
							>
								<Select.Trigger id="subject-grade-filter" aria-label="กรองระดับชั้น" class="w-full"
									>{gradeFilter === allValue
										? 'ทุกระดับชั้น'
										: (gradeLevelOptions.find((level) => level.id === gradeFilter)?.name ??
											'ทุกระดับชั้น')}</Select.Trigger
								>
								<Select.Content
									><Select.Item value={allValue}>ทุกระดับชั้น</Select.Item
									>{#each gradeLevelOptions as option (option.id)}<Select.Item value={option.id}
											>{option.name}</Select.Item
										>{/each}</Select.Content
								>
							</Select.Root>
						</div>
						<div class="space-y-1.5">
							<Label for="subject-state-filter">สถานะ</Label>
							<Select.Root type="single" bind:value={() => stateFilter, selectState}>
								<Select.Trigger id="subject-state-filter" aria-label="กรองสถานะ" class="w-full"
									>{stateFilter === allValue
										? 'ทุกสถานะ'
										: displayStateLabel(stateFilter)}</Select.Trigger
								>
								<Select.Content
									><Select.Item value={allValue}>ทุกสถานะ</Select.Item
									>{#each CATALOG_DISPLAY_STATE_OPTIONS as option (option.value)}<Select.Item
											value={option.value}>{option.label}</Select.Item
										>{/each}</Select.Content
								>
							</Select.Root>
						</div>
					</div>
				{/if}
				{#if activeFilterLabels.length}<p class="text-xs text-muted-foreground">
						{activeFilterLabels.join(' · ')}
					</p>{/if}
			</section>

			<section class="min-w-0 overflow-hidden rounded-xl border bg-card" aria-label="รายการรายวิชา">
				<div class="flex flex-wrap items-center justify-between gap-2 border-b px-3 py-2 sm:px-4">
					<h2 class="text-sm font-semibold">รายการรายวิชา</h2>
					<span class="text-xs text-muted-foreground"
						>{filteredSubjects.length} จาก {subjectItems.length} รายวิชา</span
					>
				</div>
				{#if subjectItems.length === 0}
					<PageState
						title="ยังไม่มีรายวิชาในทะเบียน"
						description={canCreate
							? 'เลือกเพิ่มรายวิชา แล้วสร้างรายละเอียดรุ่นแรกเมื่อพร้อม'
							: 'รายวิชาที่คุณมีสิทธิ์ดูจะแสดงที่นี่'}
					/>
				{:else if filteredSubjects.length === 0}
					<PageState
						title="ไม่พบรายวิชาที่ตรงกับตัวกรอง"
						description="ลองเปลี่ยนคำค้นหาหรือล้างตัวกรอง"
						actionLabel="ล้างตัวกรอง"
						onaction={clearFilters}
					/>
				{:else}
					<div class="hidden md:block">
						<Table.Root class="min-w-[1120px]" aria-label="ทะเบียนรายวิชา">
							<Table.Header
								><Table.Row
									><Table.Head scope="col">รหัส / ชื่อรายวิชา</Table.Head><Table.Head scope="col"
										>กลุ่มสาระ</Table.Head
									><Table.Head scope="col">ประเภท</Table.Head><Table.Head scope="col"
										>ระดับชั้น</Table.Head
									><Table.Head scope="col" class="text-end">หน่วยกิต</Table.Head><Table.Head
										scope="col">ภาระการเรียน</Table.Head
									><Table.Head scope="col">สถานะ</Table.Head><Table.Head scope="col"
										><span class="sr-only">รายละเอียด</span></Table.Head
									></Table.Row
								></Table.Header
							>
							<Table.Body
								>{#each visibleSubjects as item (item.subject.id)}<Table.Row>
										<Table.Cell class="min-w-60 whitespace-normal"
											><Button
												variant="link"
												class="h-auto justify-start p-0 text-start"
												onclick={() => openSubject(item)}>{item.subject.code}</Button
											>
											<p class="font-medium">
												{item.displayVersion?.nameTh ?? 'ยังไม่มีรายละเอียดรุ่น'}
											</p>
											{#if item.displayVersion?.nameEn}<p class="text-xs text-muted-foreground">
													{item.displayVersion.nameEn}
												</p>{/if}{#if item.displayVersion}<p
													class="mt-1 text-xs text-muted-foreground"
												>
													v{item.displayVersion.versionNo} · {formatEffectiveRange(
														item.displayVersion.effectiveFrom,
														item.displayVersion.effectiveUntil
													)}
												</p>{/if}</Table.Cell
										>
										<Table.Cell class="min-w-36 whitespace-normal"
											>{subjectGroupName(item.subject.subjectGroupId)}</Table.Cell
										>
										<Table.Cell class="min-w-32 whitespace-normal"
											>{optionLabel(
												SUBJECT_TYPE_OPTIONS,
												item.displayVersion?.subjectType
											)}</Table.Cell
										>
										<Table.Cell class="min-w-28 whitespace-normal"
											>{gradeLevelSummary(item.gradeLevels)}</Table.Cell
										>
										<Table.Cell class="text-end tabular-nums"
											>{item.displayVersion?.credit ?? '—'}</Table.Cell
										>
										<Table.Cell class="min-w-36 whitespace-normal"
											>{#if item.displayVersion}<p class="tabular-nums">
													{item.displayVersion.periodsPerWeek ?? '—'} คาบ/สัปดาห์
												</p>
												<p class="text-xs text-muted-foreground">
													{item.displayVersion.hoursPerSemester ?? '—'} ชม./ภาคเรียน
												</p>{:else}—{/if}</Table.Cell
										>
										<Table.Cell
											><div class="flex flex-wrap gap-1.5">
												<Badge variant="outline" class={displayStateClass(item.displayState)}
													>{displayStateLabel(item.displayState)}</Badge
												>{#if item.draftCount > 0}<Badge variant="secondary"
														>ร่าง {item.draftCount}</Badge
													>{/if}{#if item.subject.archivedAt}<Badge variant="secondary"
														>เก็บถาวร</Badge
													>{/if}
											</div></Table.Cell
										>
										<Table.Cell
											><Button
												variant="ghost"
												size="icon-sm"
												aria-label={`เปิด ${item.subject.code}`}
												onclick={() => openSubject(item)}
												><ArrowUpRight class="size-4" aria-hidden="true" /></Button
											></Table.Cell
										>
									</Table.Row>{/each}</Table.Body
							>
						</Table.Root>
					</div>
					<ul class="divide-y md:hidden" aria-label="รายวิชา">
						{#each visibleSubjects as item (item.subject.id)}<li class="space-y-2 p-3">
								<div class="flex items-start justify-between gap-3">
									<div class="min-w-0">
										<Button
											variant="link"
											class="h-auto justify-start p-0 text-start"
											onclick={() => openSubject(item)}>{item.subject.code}</Button
										>
										<h3 class="text-sm font-medium">
											{item.displayVersion?.nameTh ?? 'ยังไม่มีรายละเอียดรุ่น'}
										</h3>
										{#if item.displayVersion?.nameEn}<p class="text-xs text-muted-foreground">
												{item.displayVersion.nameEn}
											</p>{/if}
									</div>
									<Button
										variant="ghost"
										size="icon-sm"
										aria-label={`เปิด ${item.subject.code}`}
										onclick={() => openSubject(item)}
										><ArrowUpRight class="size-4" aria-hidden="true" /></Button
									>
								</div>
								<p class="text-xs text-muted-foreground">
									{subjectGroupName(item.subject.subjectGroupId)} · {optionLabel(
										SUBJECT_TYPE_OPTIONS,
										item.displayVersion?.subjectType
									)}
								</p>
								<dl class="flex flex-wrap gap-x-4 gap-y-1 text-xs">
									<div class="flex gap-1">
										<dt class="text-muted-foreground">หน่วยกิต</dt>
										<dd>{item.displayVersion?.credit ?? '—'}</dd>
									</div>
									<div class="flex gap-1">
										<dt class="text-muted-foreground">คาบ/สัปดาห์</dt>
										<dd>{item.displayVersion?.periodsPerWeek ?? '—'}</dd>
									</div>
									<div class="flex gap-1">
										<dt class="text-muted-foreground">ชม./ภาคเรียน</dt>
										<dd>{item.displayVersion?.hoursPerSemester ?? '—'}</dd>
									</div>
								</dl>
								<p class="text-xs">{gradeLevelSummary(item.gradeLevels)}</p>
								{#if item.displayVersion}<p class="text-xs text-muted-foreground">
										v{item.displayVersion.versionNo} · {formatEffectiveRange(
											item.displayVersion.effectiveFrom,
											item.displayVersion.effectiveUntil
										)}
									</p>{/if}
								<div class="flex flex-wrap gap-1.5">
									<Badge variant="outline" class={displayStateClass(item.displayState)}
										>{displayStateLabel(item.displayState)}</Badge
									>{#if item.draftCount > 0}<Badge variant="secondary">ร่าง {item.draftCount}</Badge
										>{/if}{#if item.subject.archivedAt}<Badge variant="secondary">เก็บถาวร</Badge
										>{/if}
								</div>
							</li>{/each}
					</ul>
					{#if pageCount > 1}<nav
							class="flex flex-wrap items-center justify-between gap-3 border-t p-3"
							aria-label="หน้ารายวิชา"
						>
							<span class="text-xs text-muted-foreground"
								>หน้า {pageNumber} / {pageCount} · {(pageNumber - 1) * pageSize + 1}–{Math.min(
									pageNumber * pageSize,
									filteredSubjects.length
								)} จาก {filteredSubjects.length}</span
							>
							<div class="flex gap-2">
								<Button
									variant="outline"
									size="sm"
									disabled={pageNumber === 1}
									onclick={() => (pageNumber -= 1)}>ก่อนหน้า</Button
								><Button
									variant="outline"
									size="sm"
									disabled={pageNumber === pageCount}
									onclick={() => (pageNumber += 1)}>ถัดไป</Button
								>
							</div>
						</nav>{/if}
				{/if}
			</section>
			<p class="text-xs text-muted-foreground">
				ข้อมูลกลางรายวิชา ไม่ผูกกับภาคเรียนที่เลือกบนแถบด้านบน
			</p>
			{#if errorMessage}<div
					role="alert"
					class="flex flex-wrap items-center gap-2 text-sm text-destructive"
				>
					<p>{errorMessage}</p>
					<Button size="sm" variant="outline" onclick={loadOverview}>ลองอีกครั้ง</Button>
				</div>{/if}
		</div>
	{/if}
</PageShell>

<Sheet.Root bind:open={() => sheetOpen, setSheetOpen}>
	<Sheet.Content class="w-full overflow-y-auto p-4 sm:max-w-5xl sm:p-6">
		<Sheet.Header class="pe-8 text-start"
			><Sheet.Title class="flex flex-wrap items-center gap-2"
				><BookOpen class="size-5 shrink-0 text-primary" aria-hidden="true" /><span
					>{selected?.subject.code ?? 'รายวิชา'}</span
				>{#if selected?.displayVersion}<span>· {selected.displayVersion.nameTh}</span
					>{/if}</Sheet.Title
			><Sheet.Description
				>{selected?.displayVersion
					? formatEffectiveRange(
							selected.displayVersion.effectiveFrom,
							selected.displayVersion.effectiveUntil
						)
					: 'ยังไม่มีรายละเอียดรุ่น สร้างรุ่นแรกได้ด้านล่าง'}</Sheet.Description
			></Sheet.Header
		>
		{#if selected}
			{#key selected.subject.id}
				<CatalogDeleteDialog
					kind="subject"
					name={selected.displayVersion
						? `${selected.subject.code} · ${selected.displayVersion.nameTh}`
						: selected.subject.code}
					canDelete={selected.canDelete}
					onDelete={deleteSubject}
				/>
			{/key}
			<div class="flex flex-wrap gap-2">
				<Badge variant="outline" class={displayStateClass(selected.displayState)}
					>{displayStateLabel(selected.displayState)}</Badge
				>{#if selected.displayVersion}<Badge variant="secondary"
						>v{selected.displayVersion.versionNo}</Badge
					>{/if}{#if selected.subject.archivedAt}<Badge variant="secondary">เก็บถาวร</Badge>{/if}
			</div>
			<section
				class="space-y-3 rounded-xl border bg-card p-3 sm:p-4"
				aria-label="กลุ่มสาระของรายวิชา"
			>
				{#if selected.canManage}<div class="flex flex-col gap-3 sm:flex-row sm:items-end">
						<div class="min-w-0 flex-1 space-y-1.5">
							<Label for="subject-affiliation">กลุ่มสาระของรายวิชา</Label><Select.Root
								type="single"
								bind:value={editingSubjectGroupValue}
								disabled={subjectMetadataSaving}
								><Select.Trigger id="subject-affiliation" class="w-full"
									>{subjectGroupName(editingSubjectGroupValue)}</Select.Trigger
								><Select.Content
									>{#each manageableSubjectGroupOptions as option (option.subjectGroupId)}<Select.Item
											value={option.subjectGroupId}>{option.name}</Select.Item
										>{/each}</Select.Content
								></Select.Root
							>
						</div>
						<LoadingButton
							type="button"
							variant="outline"
							loading={subjectMetadataSaving}
							loadingLabel="กำลังบันทึก..."
							disabled={editingSubjectGroupValue === selected.subject.subjectGroupId}
							onclick={saveSubjectGroup}
							><Save class="size-4" aria-hidden="true" />บันทึกกลุ่มสาระ</LoadingButton
						>
					</div>
				{:else}<dl>
						<dt class="text-xs text-muted-foreground">กลุ่มสาระของรายวิชา</dt>
						<dd class="mt-1 text-sm font-medium">
							{subjectGroupName(selected.subject.subjectGroupId)}
						</dd>
					</dl>{/if}
				{#if subjectMetadataError}<p role="alert" class="text-sm text-destructive">
						{subjectMetadataError}
					</p>{/if}
			</section>
			<div aria-busy={historyLoading} data-testid="subject-history-region">
				{#if historyLoading && versions.length === 0}<PageSkeleton variant="cards" rows={3} />
				{:else}
					{#if historyError}<PageState
							variant="error"
							title="โหลดประวัติไม่สำเร็จ"
							description={historyError}
							actionLabel="ลองอีกครั้ง"
							onaction={() => reloadSelectedHistory(false)}
						/>{/if}
					{#if !historyError || versions.length > 0}{#key selected.subject.id}<CatalogVersionHistory
								kind="subject"
								sourceVersion={subjectVersionSource(selected.displayVersion, versions)}
								code={selected.subject.code}
								items={versions.map((item) => ({
									id: item.id,
									versionNo: item.versionNo,
									name: item.nameTh,
									secondaryName: item.nameEn,
									exactValue: item.credit,
									totalValue: item.hoursPerSemester?.toString() ?? '',
									standardPeriodsPerWeek: item.periodsPerWeek,
									effectiveFrom: item.effectiveFrom,
									effectiveUntil: item.effectiveUntil,
									classification: item.subjectType,
									gradeLevels: gradeLevelsFor(item),
									status: item.status,
									rowVersion: item.rowVersion
								}))}
								{gradeLevelOptions}
								canManage={selected.canManage}
								onCreate={addVersion}
								onPublish={publish}
							/>{/key}{/if}
				{/if}
			</div>
		{/if}
	</Sheet.Content>
</Sheet.Root>

<Dialog.Root bind:open={() => createOpen, setCreateOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header
			><Dialog.Title>เพิ่มรายวิชา</Dialog.Title><Dialog.Description
				>เพิ่มรหัสและกลุ่มสาระ แล้วกำหนดรายละเอียดรายวิชาในรุ่นแรก</Dialog.Description
			></Dialog.Header
		>
		<form class="space-y-4" onsubmit={addSubject}>
			<div class="space-y-1.5">
				<Label for="new-subject-code">เพิ่มรหัสรายวิชา</Label><Input
					id="new-subject-code"
					class="uppercase"
					bind:value={code}
					placeholder="เช่น ค21101"
					required
					disabled={creating}
				/>
			</div>
			<div class="space-y-1.5">
				<Label for="new-subject-group">กลุ่มสาระ</Label><Select.Root
					type="single"
					bind:value={subjectGroupValue}
					disabled={creating}
					><Select.Trigger id="new-subject-group" class="w-full"
						>{selectedSubjectGroup?.name ?? 'เลือกกลุ่มสาระ'}</Select.Trigger
					><Select.Content
						>{#each manageableSubjectGroupOptions as option (option.subjectGroupId)}<Select.Item
								value={option.subjectGroupId}>{option.name}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
			</div>
			{#if mutationError}<p role="alert" class="text-sm text-destructive">{mutationError}</p>{/if}
			<Dialog.Footer
				><Button variant="outline" onclick={() => setCreateOpen(false)}>ยกเลิก</Button
				><LoadingButton type="submit" loading={creating} loadingLabel="กำลังเพิ่ม..."
					>เพิ่มและกำหนดรายละเอียด</LoadingButton
				></Dialog.Footer
			>
		</form>
	</Dialog.Content>
</Dialog.Root>
