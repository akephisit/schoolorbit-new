<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		createActivityVersion,
		createCatalogActivity,
		listActivityVersions,
		publishActivityVersion,
		type ActivityVersion,
		type CatalogActivityOverview,
		type CatalogActivityOverviewItem,
		type CatalogDisplayState
	} from '#lib/api/academic-core.js';
	import { CATALOG_ACTIVITY_OVERVIEW_DEPENDENCY } from '#lib/academic-core/catalog-route.js';
	import {
		ACTIVITY_TYPE_OPTIONS,
		CATALOG_DISPLAY_STATE_OPTIONS,
		SCHEDULING_MODE_OPTIONS,
		displayStateClass,
		displayStateLabel,
		formatEffectiveRange,
		gradeLevelSummary,
		matchesCatalogSearch,
		optionLabel
	} from '#lib/academic-core/catalog-presentation.js';
	import CatalogVersionHistory from '#lib/components/academic-core/CatalogVersionHistory.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import {
		LoadingButton,
		PageSkeleton,
		PageState,
		RegionUpdatingState
	} from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Sheet from '#lib/components/ui/sheet/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { ArrowUpRight, Plus, Search, SlidersHorizontal, Sparkles } from '@lucide/svelte';
	import type { PageProps } from './$types';

	type VersionDraft = {
		name: string;
		secondaryName: string;
		exactValue: string;
		totalValue: string;
		standardPeriodsPerWeek: string;
		effectiveFrom: string;
		effectiveUntil: string;
		gradeLevelIds: string[];
		classification: string;
	};

	const allValue = 'all';
	const collator = new Intl.Collator('th-TH', { numeric: true, sensitivity: 'base' });
	const historyRequests = new LatestRequest();
	let disposed = false;
	onDestroy(() => {
		disposed = true;
		historyRequests.abort();
	});

	let { data }: PageProps = $props();
	let overview = $state.raw<CatalogActivityOverview | null>(null);
	let selected = $state<CatalogActivityOverviewItem | null>(null);
	let versions = $state.raw<ActivityVersion[]>([]);
	let sheetOpen = $state(false);
	let historyLoading = $state(false);
	let historyError = $state('');
	let loading = $state(true);
	let errorMessage = $state('');
	let mutationError = $state('');
	let creating = $state(false);
	let createOpen = $state(false);
	let filtersOpen = $state(false);
	let activityType = $state(ACTIVITY_TYPE_OPTIONS[0].value);
	let search = $state('');
	let typeFilter = $state(allValue);
	let schedulingFilter = $state(allValue);
	let gradeFilter = $state(allValue);
	let stateFilter = $state<CatalogDisplayState | typeof allValue>(allValue);
	let hasFilters = $derived(
		search.trim() !== '' ||
			[typeFilter, schedulingFilter, gradeFilter, stateFilter].some((value) => value !== allValue)
	);
	let advancedFilterCount = $derived(
		[schedulingFilter, gradeFilter, stateFilter].filter((value) => value !== allValue).length
	);

	$effect(() => {
		if (!sheetOpen) historyRequests.abort();
	});

	let activityItems = $derived(overview?.items ?? []);
	let gradeLevelOptions = $derived(overview?.gradeLevelOptions ?? []);
	let canCreate = $derived(overview?.canCreate ?? false);
	let canOpenDelivery = $derived($can.hasModule(PERMISSION_MODULES.LEARNING_OFFERING));
	let activityTypeOptions = $derived.by(() => {
		const known = new Set<string>(ACTIVITY_TYPE_OPTIONS.map((option) => option.value));
		const legacy = activityItems
			.map((item) => item.activity.activityType)
			.filter((value, index, values) => !known.has(value) && values.indexOf(value) === index)
			.map((value) => ({ value, label: value }));
		return [...ACTIVITY_TYPE_OPTIONS, ...legacy];
	});
	let filteredActivities = $derived.by(() =>
		activityItems
			.filter((item) => {
				const version = item.displayVersion;
				return (
					matchesCatalogSearch(
						search,
						version?.name,
						version?.description,
						optionLabel(activityTypeOptions, item.activity.activityType)
					) &&
					(typeFilter === allValue || item.activity.activityType === typeFilter) &&
					(schedulingFilter === allValue || version?.schedulingMode === schedulingFilter) &&
					(gradeFilter === allValue ||
						item.gradeLevels.some((level) => level.id === gradeFilter)) &&
					(stateFilter === allValue || item.displayState === stateFilter)
				);
			})
			.sort(
				(left, right) =>
					collator.compare(activityName(left), activityName(right)) ||
					left.activity.id.localeCompare(right.activity.id)
			)
	);

	function loadOverview() {
		return invalidate(CATALOG_ACTIVITY_OVERVIEW_DEPENDENCY);
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
						const latest = result.data;
						const selectedId = selected?.activity.id;
						overview = latest;
						if (selectedId) {
							selected = latest.items.find((item) => item.activity.id === selectedId) ?? null;
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

	function activityName(item: CatalogActivityOverviewItem) {
		return (
			item.displayVersion?.name ??
			`${optionLabel(activityTypeOptions, item.activity.activityType)} · ยังไม่ได้ตั้งชื่อ`
		);
	}

	function clearFilters() {
		search = '';
		typeFilter = schedulingFilter = gradeFilter = stateFilter = allValue;
	}

	async function openActivity(item: CatalogActivityOverviewItem) {
		const request = historyRequests.begin();
		selected = item;
		sheetOpen = true;
		historyError = '';
		versions = [];
		historyLoading = true;
		try {
			const loaded = await listActivityVersions(item.activity.id, { signal: request.signal });
			if (historyRequests.isCurrent(request.revision)) versions = loaded;
		} catch (error) {
			if (historyRequests.isCurrent(request.revision) && !isAbortError(error)) {
				historyError = error instanceof Error ? error.message : 'โหลดประวัติกิจกรรมไม่สำเร็จ';
			}
		} finally {
			if (historyRequests.isCurrent(request.revision)) historyLoading = false;
		}
	}

	async function reloadSelectedHistory() {
		if (!selected) return;
		await openActivity(selected);
		await loadOverview();
	}

	async function addActivity(event: SubmitEvent) {
		event.preventDefault();
		if (creating || !canCreate || !overview) return;
		creating = true;
		mutationError = '';
		try {
			const created = await createCatalogActivity({ activityType });
			if (disposed) return;
			const item: CatalogActivityOverviewItem = {
				activity: created,
				displayVersion: null,
				displayState: 'unpublished',
				draftCount: 0,
				gradeLevels: [],
				canManage: true
			};
			overview = { ...overview, items: [...overview.items, item] };
			createOpen = false;
			void openActivity(item);
		} catch (error) {
			mutationError = error instanceof Error ? error.message : 'เพิ่มกิจกรรมไม่สำเร็จ';
		} finally {
			creating = false;
		}
	}

	async function addVersion(draft: VersionDraft) {
		if (!selected) return;
		const activityId = selected.activity.id;
		await createActivityVersion(activityId, {
			name: draft.name,
			hoursPerWeek: draft.exactValue,
			hoursPerTerm: draft.totalValue,
			description: draft.secondaryName || null,
			effectiveFrom: draft.effectiveFrom,
			effectiveUntil: draft.effectiveUntil || null,
			gradeLevelIds: draft.gradeLevelIds,
			schedulingMode: draft.classification,
			termCode: null
		});
		if (disposed) return;
		if (sheetOpen && selected?.activity.id === activityId) await reloadSelectedHistory();
		else await loadOverview();
	}

	async function publish(id: string, rowVersion: number) {
		const activityId = selected?.activity.id;
		await publishActivityVersion(id, { rowVersion });
		if (disposed) return;
		if (sheetOpen && selected?.activity.id === activityId) await reloadSelectedHistory();
		else await loadOverview();
	}

	function gradeLevelsFor(version: ActivityVersion) {
		return gradeLevelOptions.filter((level) => version.gradeLevelIds.includes(level.id));
	}
</script>

<PageShell
	title="ทะเบียนกิจกรรม"
	description="ข้อมูลกลางกิจกรรมพัฒนาผู้เรียน สำหรับนำไปใช้ในหลักสูตรและเปิดกิจกรรมแต่ละภาคเรียน"
>
	{#snippet actions()}
		{#if canOpenDelivery}
			<Button href="/staff/academic/delivery?kind=activity" variant="outline">
				เปิดกิจกรรมในภาคเรียน <ArrowUpRight class="size-4" />
			</Button>
		{/if}
		{#if canCreate}
			<Button
				onclick={() => {
					mutationError = '';
					createOpen = true;
				}}><Plus class="size-4" />เพิ่มกิจกรรม</Button
			>
		{/if}
	{/snippet}

	{#if loading && !overview}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && !overview}
		<PageState
			variant="error"
			title="โหลดทะเบียนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadOverview()}
		/>
	{:else}
		<div
			class="relative space-y-4"
			aria-label="ทะเบียนกิจกรรม"
			aria-busy={loading}
			data-testid="catalog-activities-ready"
		>
			{#if loading}<RegionUpdatingState label="กำลังอัปเดตทะเบียนกิจกรรม" />{/if}
			<section aria-label="ค้นหาและกรองกิจกรรม" class="rounded-xl border bg-card p-3 sm:p-4">
				<div class="space-y-3">
					<div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(180px,240px)_auto]">
						<div class="relative">
							<Search
								class="absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
							/>
							<Label class="sr-only" for="activity-search">ค้นหากิจกรรม</Label>
							<Input
								id="activity-search"
								class="ps-9"
								bind:value={search}
								placeholder="ค้นหาชื่อหรือรายละเอียดกิจกรรม"
							/>
						</div>
						<Select.Root type="single" bind:value={typeFilter}>
							<Select.Trigger aria-label="กรองประเภทกิจกรรม" class="w-full">
								{typeFilter === allValue
									? 'ทุกประเภทกิจกรรม'
									: optionLabel(activityTypeOptions, typeFilter)}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value={allValue}>ทุกประเภทกิจกรรม</Select.Item>
								{#each activityTypeOptions as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						<Button
							variant="outline"
							aria-expanded={filtersOpen}
							aria-controls="activity-advanced-filters"
							onclick={() => (filtersOpen = !filtersOpen)}
						>
							<SlidersHorizontal
								class="size-4"
							/>ตัวกรองเพิ่มเติม{#if advancedFilterCount > 0}<Badge variant="secondary"
									>{advancedFilterCount}</Badge
								>{/if}
						</Button>
					</div>
				</div>

				{#if filtersOpen}
					<div id="activity-advanced-filters" class="grid gap-3 border-t pt-3 sm:grid-cols-3">
						<Select.Root type="single" bind:value={schedulingFilter}>
							<Select.Trigger aria-label="กรองรูปแบบการจัด" class="w-full">
								{schedulingFilter === allValue
									? 'ทุกรูปแบบการจัด'
									: optionLabel(SCHEDULING_MODE_OPTIONS, schedulingFilter)}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value={allValue}>ทุกรูปแบบการจัด</Select.Item>
								{#each SCHEDULING_MODE_OPTIONS as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						<Select.Root type="single" bind:value={gradeFilter}>
							<Select.Trigger aria-label="กรองระดับชั้น" class="w-full">
								{gradeFilter === allValue
									? 'ทุกระดับชั้น'
									: (gradeLevelOptions.find((level) => level.id === gradeFilter)?.name ??
										'ทุกระดับชั้น')}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value={allValue}>ทุกระดับชั้น</Select.Item>
								{#each gradeLevelOptions as option (option.id)}
									<Select.Item value={option.id}>{option.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						<Select.Root type="single" bind:value={stateFilter}>
							<Select.Trigger aria-label="กรองสถานะ" class="w-full">
								{stateFilter === allValue ? 'ทุกสถานะ' : displayStateLabel(stateFilter)}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value={allValue}>ทุกสถานะ</Select.Item>
								{#each CATALOG_DISPLAY_STATE_OPTIONS as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
				{/if}
			</section>

			<Card.Root class="gap-0 overflow-hidden py-0">
				<div class="flex flex-wrap items-center justify-between gap-3 border-b px-4 py-3 sm:px-5">
					<div class="flex items-center gap-2">
						<Sparkles class="size-4 text-primary" />
						<h2 class="font-semibold">รายการกิจกรรม</h2>
						<Badge variant="secondary">{filteredActivities.length}</Badge>
					</div>
					{#if hasFilters}<Button variant="ghost" size="sm" onclick={clearFilters}
							>ล้างตัวกรอง</Button
						>{/if}
				</div>

				{#if activityItems.length === 0}
					<PageState
						class="m-4 border-0 shadow-none"
						title="ยังไม่มีกิจกรรมในทะเบียน"
						description={canCreate
							? 'เลือกเพิ่มกิจกรรม แล้วกำหนดชื่อ ชั่วโมง และระดับชั้นในรายละเอียด'
							: 'กิจกรรมที่โรงเรียนเพิ่มจะแสดงที่นี่'}
					/>
				{:else if filteredActivities.length === 0}
					<PageState
						class="m-4 border-0 shadow-none"
						title="ไม่พบกิจกรรมที่ตรงกับตัวกรอง"
						description="ลองเปลี่ยนคำค้นหาหรือล้างตัวกรองเพื่อดูรายการทั้งหมด"
						actionLabel="ล้างตัวกรอง"
						onaction={clearFilters}
					/>
				{:else}
					<div class="hidden md:block">
						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head class="ps-5">ชื่อกิจกรรม</Table.Head>
									<Table.Head>ประเภทกิจกรรม</Table.Head>
									<Table.Head>รูปแบบการจัด</Table.Head>
									<Table.Head>ระดับชั้น</Table.Head>
									<Table.Head class="text-end">ชม./สัปดาห์</Table.Head>
									<Table.Head>สถานะ</Table.Head>
									<Table.Head class="w-12"><span class="sr-only">เปิดรายละเอียด</span></Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each filteredActivities as item (item.activity.id)}
									<Table.Row>
										<Table.Cell class="max-w-[280px] whitespace-normal ps-5">
											<Button
												variant="link"
												class="h-auto justify-start whitespace-normal p-0 text-start"
												onclick={() => openActivity(item)}>{activityName(item)}</Button
											>
											{#if item.displayVersion?.description}<p
													class="line-clamp-1 text-xs text-muted-foreground"
												>
													{item.displayVersion.description}
												</p>{/if}
											{#if item.displayVersion}
												<p class="mt-1 text-xs text-muted-foreground">
													{formatEffectiveRange(
														item.displayVersion.effectiveFrom,
														item.displayVersion.effectiveUntil
													)}
												</p>
											{/if}
										</Table.Cell>
										<Table.Cell class="whitespace-normal"
											>{optionLabel(activityTypeOptions, item.activity.activityType)}</Table.Cell
										>
										<Table.Cell
											>{optionLabel(
												SCHEDULING_MODE_OPTIONS,
												item.displayVersion?.schedulingMode
											)}</Table.Cell
										>
										<Table.Cell class="max-w-[190px] whitespace-normal"
											>{gradeLevelSummary(item.gradeLevels)}</Table.Cell
										>
										<Table.Cell class="text-end font-mono tabular-nums"
											>{item.displayVersion?.hoursPerWeek ?? '—'}</Table.Cell
										>
										<Table.Cell>
											<div class="flex flex-wrap gap-1.5">
												<Badge variant="outline" class={displayStateClass(item.displayState)}
													>{displayStateLabel(item.displayState)}</Badge
												>
												{#if item.draftCount > 0}<Badge variant="secondary"
														>ร่าง {item.draftCount}</Badge
													>{/if}
												{#if item.activity.archivedAt}<Badge variant="secondary">เก็บถาวร</Badge
													>{/if}
											</div>
										</Table.Cell>
										<Table.Cell>
											<Button
												variant="ghost"
												size="icon"
												aria-label={`เปิดรายละเอียด ${activityName(item)}`}
												onclick={() => openActivity(item)}><ArrowUpRight class="size-4" /></Button
											>
										</Table.Cell>
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
					</div>

					<div class="grid gap-3 p-4 md:hidden">
						{#each filteredActivities as item (item.activity.id)}
							<Button
								variant="ghost"
								class="h-auto w-full flex-col items-stretch gap-0 whitespace-normal rounded-xl border bg-card p-4 text-start"
								onclick={() => openActivity(item)}
							>
								<div class="flex items-start justify-between gap-3">
									<div class="min-w-0">
										<h3 class="font-semibold">
											{activityName(item)}
										</h3>
									</div>
									<ArrowUpRight class="size-4 shrink-0 text-muted-foreground" />
								</div>
								<div class="mt-3 grid grid-cols-2 gap-3 text-sm">
									<div>
										<p class="text-xs text-muted-foreground">ประเภทกิจกรรม</p>
										<p>{optionLabel(activityTypeOptions, item.activity.activityType)}</p>
									</div>
									<div>
										<p class="text-xs text-muted-foreground">รูปแบบการจัด</p>
										<p>
											{optionLabel(SCHEDULING_MODE_OPTIONS, item.displayVersion?.schedulingMode)}
										</p>
									</div>
									<div>
										<p class="text-xs text-muted-foreground">ชั่วโมงต่อสัปดาห์</p>
										<p class="font-mono">{item.displayVersion?.hoursPerWeek ?? '—'}</p>
									</div>
									<div>
										<p class="text-xs text-muted-foreground">ระดับชั้น</p>
										<p>{gradeLevelSummary(item.gradeLevels)}</p>
									</div>
									{#if item.displayVersion}
										<div class="col-span-2">
											<p class="text-xs text-muted-foreground">ช่วงที่มีผล</p>
											<p>
												{formatEffectiveRange(
													item.displayVersion.effectiveFrom,
													item.displayVersion.effectiveUntil
												)}
											</p>
										</div>
									{/if}
								</div>
								<div class="mt-3 flex flex-wrap gap-1.5">
									<Badge variant="outline" class={displayStateClass(item.displayState)}
										>{displayStateLabel(item.displayState)}</Badge
									>{#if item.draftCount > 0}<Badge variant="secondary">ร่าง {item.draftCount}</Badge
										>{/if}
									{#if item.activity.archivedAt}<Badge variant="secondary">เก็บถาวร</Badge>{/if}
								</div>
							</Button>
						{/each}
					</div>
				{/if}
			</Card.Root>

			<p class="text-xs text-muted-foreground">
				แสดง {filteredActivities.length} จาก {activityItems.length} กิจกรรม
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

<Sheet.Root bind:open={sheetOpen}>
	<Sheet.Content class="w-full overflow-y-auto sm:max-w-5xl">
		<Sheet.Header class="pe-8 text-start">
			<Sheet.Title class="flex items-center gap-2">
				<Sparkles class="size-5 text-primary" />
				<span>{selected ? activityName(selected) : 'รายละเอียดกิจกรรม'}</span>
			</Sheet.Title>
			<Sheet.Description>
				{#if selected?.displayVersion}{formatEffectiveRange(
						selected.displayVersion.effectiveFrom,
						selected.displayVersion.effectiveUntil
					)}{:else}ยังไม่มีรายละเอียดรุ่น เลือกสร้างรุ่นแรกได้ด้านล่าง{/if}
			</Sheet.Description>
		</Sheet.Header>
		{#if historyLoading}
			<PageSkeleton variant="cards" rows={3} />
		{:else if historyError}
			<PageState
				variant="error"
				title="โหลดประวัติไม่สำเร็จ"
				description={historyError}
				actionLabel="ลองอีกครั้ง"
				onaction={() => selected && openActivity(selected)}
			/>
		{:else if selected}
			{#key selected.activity.id}
				<CatalogVersionHistory
					kind="activity"
					code={selected.activity.code}
					items={versions.map((item) => ({
						id: item.id,
						versionNo: item.versionNo,
						name: item.name,
						secondaryName: item.description,
						exactValue: item.hoursPerWeek,
						totalValue: item.hoursPerTerm,
						effectiveFrom: item.effectiveFrom,
						effectiveUntil: item.effectiveUntil,
						classification: item.schedulingMode,
						gradeLevels: gradeLevelsFor(item),
						status: item.status,
						rowVersion: item.rowVersion
					}))}
					{gradeLevelOptions}
					canManage={selected.canManage}
					onCreate={addVersion}
					onPublish={publish}
				/>
			{/key}
		{/if}
	</Sheet.Content>
</Sheet.Root>

<Dialog.Root bind:open={createOpen}>
	<Dialog.Content
		class="sm:max-w-md"
		onInteractOutside={(event) => {
			if (creating) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (creating) event.preventDefault();
		}}
		showCloseButton={!creating}
	>
		<Dialog.Header>
			<Dialog.Title>เพิ่มกิจกรรม</Dialog.Title>
			<Dialog.Description
				>เลือกประเภทกิจกรรม จากนั้นกำหนดชื่อ ชั่วโมง และระดับชั้นในรายละเอียดกิจกรรม</Dialog.Description
			>
		</Dialog.Header>
		<form class="space-y-4" onsubmit={addActivity}>
			<div class="space-y-1.5">
				<Label for="new-activity-type">ประเภทกิจกรรม <span class="text-destructive">*</span></Label>
				<Select.Root type="single" bind:value={activityType} disabled={creating}>
					<Select.Trigger id="new-activity-type" class="w-full"
						>{optionLabel(ACTIVITY_TYPE_OPTIONS, activityType)}</Select.Trigger
					>
					<Select.Content
						>{#each ACTIVITY_TYPE_OPTIONS as option (option.value)}<Select.Item value={option.value}
								>{option.label}</Select.Item
							>{/each}</Select.Content
					>
				</Select.Root>
			</div>
			{#if mutationError}<p role="alert" class="text-sm text-destructive">{mutationError}</p>{/if}
			<Dialog.Footer>
				<Button variant="outline" disabled={creating} onclick={() => (createOpen = false)}
					>ยกเลิก</Button
				>
				<LoadingButton type="submit" loading={creating} loadingLabel="กำลังเพิ่มกิจกรรม..."
					>เพิ่มและกำหนดรายละเอียด</LoadingButton
				>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
