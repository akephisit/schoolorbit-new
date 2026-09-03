<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		createActivityVersion,
		createCatalogActivity,
		getCatalogActivityOverview,
		listActivityVersions,
		publishActivityVersion,
		type ActivityVersion,
		type CatalogActivityOverview,
		type CatalogActivityOverviewItem,
		type CatalogDisplayState
	} from '$lib/api/academic-core';
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
	} from '$lib/academic-core/catalog-presentation';
	import CatalogVersionHistory from '$lib/components/academic-core/CatalogVersionHistory.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Sheet from '$lib/components/ui/sheet';
	import * as Table from '$lib/components/ui/table';
	import { PERMISSION_MODULES } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { ArrowUpRight, Plus, Search, SlidersHorizontal, Sparkles } from 'lucide-svelte';

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
	const activityHistoryCache = new SvelteMap<string, ActivityVersion[]>();

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
	let code = $state('');
	let activityType = $state(ACTIVITY_TYPE_OPTIONS[0].value);
	let search = $state('');
	let typeFilter = $state(allValue);
	let schedulingFilter = $state(allValue);
	let gradeFilter = $state(allValue);
	let stateFilter = $state<CatalogDisplayState | typeof allValue>(allValue);
	let historyRevision = 0;

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
					matchesCatalogSearch(search, item.activity.code, version?.name, version?.description) &&
					(typeFilter === allValue || item.activity.activityType === typeFilter) &&
					(schedulingFilter === allValue || version?.schedulingMode === schedulingFilter) &&
					(gradeFilter === allValue ||
						item.gradeLevels.some((level) => level.id === gradeFilter)) &&
					(stateFilter === allValue || item.displayState === stateFilter)
				);
			})
			.sort((left, right) => {
				const codeOrder = collator.compare(left.activity.code, right.activity.code);
				if (codeOrder !== 0) return codeOrder;
				return collator.compare(left.displayVersion?.name ?? '', right.displayVersion?.name ?? '');
			})
	);

	async function loadOverview(showLoading = true) {
		if (showLoading) loading = true;
		errorMessage = '';
		try {
			const selectedId = selected?.activity.id;
			overview = await getCatalogActivityOverview();
			if (selectedId) {
				selected = overview.items.find((item) => item.activity.id === selectedId) ?? null;
			}
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดทะเบียนกิจกรรมไม่สำเร็จ';
		} finally {
			if (showLoading) loading = false;
		}
	}

	async function openActivity(item: CatalogActivityOverviewItem) {
		const currentHistoryRevision = ++historyRevision;
		selected = item;
		sheetOpen = true;
		historyError = '';
		const cached = activityHistoryCache.get(item.activity.id);
		if (cached) {
			versions = cached;
			historyLoading = false;
			return;
		}

		versions = [];
		historyLoading = true;
		try {
			const loaded = await listActivityVersions(item.activity.id);
			activityHistoryCache.set(item.activity.id, loaded);
			if (currentHistoryRevision !== historyRevision) return;
			versions = loaded;
		} catch (error) {
			if (currentHistoryRevision === historyRevision) {
				historyError = error instanceof Error ? error.message : 'โหลดประวัติกิจกรรมไม่สำเร็จ';
			}
		} finally {
			if (currentHistoryRevision === historyRevision) historyLoading = false;
		}
	}

	async function reloadSelectedHistory() {
		if (!selected) return;
		const currentHistoryRevision = ++historyRevision;
		const activityId = selected.activity.id;
		activityHistoryCache.delete(activityId);
		historyLoading = true;
		historyError = '';
		try {
			const loaded = await listActivityVersions(activityId);
			activityHistoryCache.set(activityId, loaded);
			if (currentHistoryRevision !== historyRevision) return;
			versions = loaded;
			await loadOverview(false);
		} catch (error) {
			if (currentHistoryRevision === historyRevision) {
				historyError = error instanceof Error ? error.message : 'โหลดประวัติกิจกรรมไม่สำเร็จ';
			}
		} finally {
			if (currentHistoryRevision === historyRevision) historyLoading = false;
		}
	}

	async function addActivity(event: SubmitEvent) {
		event.preventDefault();
		creating = true;
		mutationError = '';
		try {
			const created = await createCatalogActivity({
				code,
				activityType
			});
			code = '';
			await loadOverview(false);
			const item = overview?.items.find((candidate) => candidate.activity.id === created.id);
			if (item) await openActivity(item);
		} catch (error) {
			mutationError = error instanceof Error ? error.message : 'เพิ่มรหัสกิจกรรมไม่สำเร็จ';
		} finally {
			creating = false;
		}
	}

	async function addVersion(draft: VersionDraft) {
		if (!selected) return;
		await createActivityVersion(selected.activity.id, {
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
		await reloadSelectedHistory();
	}

	async function publish(id: string, rowVersion: number) {
		await publishActivityVersion(id, { rowVersion });
		await reloadSelectedHistory();
	}

	function gradeLevelsFor(version: ActivityVersion) {
		return gradeLevelOptions.filter((level) => version.gradeLevelIds.includes(level.id));
	}

	onMount(() => loadOverview());
</script>

<PageShell
	title="ทะเบียนกิจกรรม"
	description="ดูกิจกรรมพัฒนาผู้เรียน ประเภท รูปแบบการจัด ระดับชั้น และรุ่นที่ใช้อยู่ได้ในหน้าเดียว"
>
	{#snippet actions()}
		{#if canOpenDelivery}
			<Button href="/staff/academic/delivery?kind=activity" variant="outline">
				เปิดกิจกรรมในภาคเรียน <ArrowUpRight class="size-4" />
			</Button>
		{/if}
	{/snippet}

	{#if loading}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && activityItems.length === 0}
		<PageState
			variant="error"
			title="โหลดทะเบียนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadOverview()}
		/>
	{:else}
		<div class="space-y-5">
			<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
				<div class="space-y-4 border-b bg-muted/25 p-4">
					<div class="flex items-center gap-2 text-sm font-medium">
						<SlidersHorizontal class="size-4 text-primary" /> ค้นหาและกรองกิจกรรม
					</div>
					<div class="grid gap-3 lg:grid-cols-[minmax(240px,1fr)_repeat(4,minmax(150px,auto))]">
						<div class="relative">
							<Search
								class="absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
							/>
							<Label class="sr-only" for="activity-search">ค้นหากิจกรรม</Label>
							<Input
								id="activity-search"
								class="ps-9"
								bind:value={search}
								placeholder="ค้นหารหัส ชื่อ หรือรายละเอียด"
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
				</div>

				{#if canCreate}
					<form
						class="grid gap-3 border-b p-4 sm:grid-cols-[minmax(160px,260px)_minmax(240px,380px)_auto] sm:items-end"
						onsubmit={addActivity}
					>
						<div>
							<Label for="new-activity-code">เพิ่มรหัสกิจกรรม</Label>
							<Input
								id="new-activity-code"
								class="mt-1.5 font-mono uppercase"
								bind:value={code}
								placeholder="เช่น GUIDE-M1"
								required
							/>
						</div>
						<div>
							<Label for="new-activity-type">ประเภทกิจกรรม</Label>
							<Select.Root type="single" bind:value={activityType}>
								<Select.Trigger id="new-activity-type" class="mt-1.5 w-full">
									{optionLabel(ACTIVITY_TYPE_OPTIONS, activityType)}
								</Select.Trigger>
								<Select.Content>
									{#each ACTIVITY_TYPE_OPTIONS as option (option.value)}
										<Select.Item value={option.value}>{option.label}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
						<Button type="submit" disabled={creating}>
							<Plus class="size-4" />
							{creating ? 'กำลังเพิ่ม...' : 'เพิ่มกิจกรรม'}
						</Button>
						{#if mutationError}
							<p role="alert" class="text-sm text-destructive sm:col-span-3">
								{mutationError}
							</p>
						{/if}
					</form>
				{/if}

				{#if activityItems.length === 0}
					<PageState
						class="m-4 border-0 shadow-none"
						title="ยังไม่มีกิจกรรมในทะเบียน"
						description="เพิ่มรหัสกิจกรรม แล้วสร้างรายละเอียดรุ่นแรกเมื่อพร้อม"
					/>
				{:else if filteredActivities.length === 0}
					<PageState
						class="m-4 border-0 shadow-none"
						title="ไม่พบกิจกรรมที่ตรงกับตัวกรอง"
						description="ลองเปลี่ยนคำค้นหา ประเภท รูปแบบการจัด ระดับชั้น หรือสถานะ"
					/>
				{:else}
					<div class="hidden md:block">
						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head class="w-[140px] ps-5">รหัส</Table.Head>
									<Table.Head>ชื่อกิจกรรม</Table.Head>
									<Table.Head>ประเภทกิจกรรม</Table.Head>
									<Table.Head>รูปแบบการจัด</Table.Head>
									<Table.Head>ระดับชั้น</Table.Head>
									<Table.Head class="text-end">ชั่วโมง</Table.Head>
									<Table.Head>สถานะ</Table.Head>
									<Table.Head class="w-12"><span class="sr-only">เปิดรายละเอียด</span></Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each filteredActivities as item (item.activity.id)}
									<Table.Row>
										<Table.Cell class="border-s-4 border-s-primary ps-5 font-mono font-semibold"
											>{item.activity.code}</Table.Cell
										>
										<Table.Cell class="max-w-[260px] whitespace-normal">
											<p class="font-medium">
												{item.displayVersion?.name ?? 'ยังไม่มีรายละเอียดรุ่น'}
											</p>
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
												aria-label={`เปิด ${item.activity.code}`}
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
							<button
								type="button"
								class="rounded-xl border border-s-4 border-s-primary bg-background p-4 text-start shadow-xs transition hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
								onclick={() => openActivity(item)}
							>
								<div class="flex items-start justify-between gap-3">
									<div class="min-w-0">
										<p class="font-mono text-sm font-semibold text-primary">{item.activity.code}</p>
										<h2 class="mt-1 font-medium">
											{item.displayVersion?.name ?? 'ยังไม่มีรายละเอียดรุ่น'}
										</h2>
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
							</button>
						{/each}
					</div>
				{/if}
			</section>

			<p class="text-xs text-muted-foreground">
				แสดง {filteredActivities.length} จาก {activityItems.length} กิจกรรม · ทะเบียนนี้เป็นข้อมูลกลาง
				ไม่ผูกกับภาคเรียนบนแถบด้านบน · สังกัดงานกิจกรรมพัฒนาผู้เรียนอัตโนมัติ
			</p>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</div>
	{/if}
</PageShell>

<Sheet.Root bind:open={sheetOpen}>
	<Sheet.Content class="w-full overflow-y-auto sm:max-w-5xl">
		<Sheet.Header class="pe-8 text-start">
			<Sheet.Title class="flex items-center gap-2">
				<Sparkles class="size-5 text-primary" />
				<span class="font-mono">{selected?.activity.code ?? 'กิจกรรม'}</span>
				{#if selected?.displayVersion}<span class="font-sans">· {selected.displayVersion.name}</span
					>{/if}
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
			<PageState variant="error" title="โหลดประวัติไม่สำเร็จ" description={historyError} />
		{:else if selected}
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
		{/if}
	</Sheet.Content>
</Sheet.Root>
