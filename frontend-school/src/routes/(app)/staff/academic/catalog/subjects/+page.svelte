<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		createCatalogSubject,
		createSubjectVersion,
		getCatalogSubjectOverview,
		listSubjectVersions,
		publishSubjectVersion,
		type CatalogDisplayState,
		type CatalogSubjectOverview,
		type CatalogSubjectOverviewItem,
		type SubjectVersion
	} from '$lib/api/academic-core';
	import {
		CATALOG_DISPLAY_STATE_OPTIONS,
		SUBJECT_TYPE_OPTIONS,
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
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { ArrowUpRight, BookOpen, Plus, Search, SlidersHorizontal } from 'lucide-svelte';

	type VersionDraft = {
		name: string;
		secondaryName: string;
		exactValue: string;
		effectiveFrom: string;
		effectiveUntil: string;
		gradeLevelIds: string[];
		classification: string;
	};

	const allValue = 'all';
	const collator = new Intl.Collator('th-TH', { numeric: true, sensitivity: 'base' });
	const subjectHistoryCache = new SvelteMap<string, SubjectVersion[]>();

	let overview = $state.raw<CatalogSubjectOverview | null>(null);
	let selected = $state<CatalogSubjectOverviewItem | null>(null);
	let versions = $state.raw<SubjectVersion[]>([]);
	let sheetOpen = $state(false);
	let historyLoading = $state(false);
	let historyError = $state('');
	let loading = $state(true);
	let errorMessage = $state('');
	let mutationError = $state('');
	let creating = $state(false);
	let code = $state('');
	let search = $state('');
	let typeFilter = $state(allValue);
	let gradeFilter = $state(allValue);
	let stateFilter = $state<CatalogDisplayState | typeof allValue>(allValue);

	const canManage = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT
		)
	);
	let subjectItems = $derived(overview?.items ?? []);
	let gradeLevelOptions = $derived(overview?.gradeLevelOptions ?? []);
	let filteredSubjects = $derived.by(() =>
		subjectItems
			.filter((item) => {
				const version = item.displayVersion;
				return (
					matchesCatalogSearch(search, item.subject.code, version?.nameTh, version?.nameEn) &&
					(typeFilter === allValue || version?.subjectType === typeFilter) &&
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

	async function loadOverview(showLoading = true) {
		if (showLoading) loading = true;
		errorMessage = '';
		try {
			const selectedId = selected?.subject.id;
			overview = await getCatalogSubjectOverview();
			if (selectedId) {
				selected = overview.items.find((item) => item.subject.id === selectedId) ?? null;
			}
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดทะเบียนรายวิชาไม่สำเร็จ';
		} finally {
			if (showLoading) loading = false;
		}
	}

	async function openSubject(item: CatalogSubjectOverviewItem) {
		selected = item;
		sheetOpen = true;
		historyError = '';
		const cached = subjectHistoryCache.get(item.subject.id);
		if (cached) {
			versions = cached;
			return;
		}

		versions = [];
		historyLoading = true;
		try {
			const loaded = await listSubjectVersions(item.subject.id);
			subjectHistoryCache.set(item.subject.id, loaded);
			if (selected?.subject.id === item.subject.id) versions = loaded;
		} catch (error) {
			historyError = error instanceof Error ? error.message : 'โหลดประวัติรายวิชาไม่สำเร็จ';
		} finally {
			historyLoading = false;
		}
	}

	async function reloadSelectedHistory() {
		if (!selected) return;
		const subjectId = selected.subject.id;
		subjectHistoryCache.delete(subjectId);
		const loaded = await listSubjectVersions(subjectId);
		subjectHistoryCache.set(subjectId, loaded);
		versions = loaded;
		await loadOverview(false);
	}

	async function addSubject(event: SubmitEvent) {
		event.preventDefault();
		creating = true;
		mutationError = '';
		try {
			const created = await createCatalogSubject({
				code,
				owningOrganizationUnitId: null
			});
			code = '';
			await loadOverview(false);
			const item = overview?.items.find((candidate) => candidate.subject.id === created.id);
			if (item) await openSubject(item);
		} catch (error) {
			mutationError = error instanceof Error ? error.message : 'เพิ่มรหัสรายวิชาไม่สำเร็จ';
		} finally {
			creating = false;
		}
	}

	async function addVersion(draft: VersionDraft) {
		if (!selected) return;
		await createSubjectVersion(selected.subject.id, {
			nameTh: draft.name,
			nameEn: draft.secondaryName || null,
			credit: draft.exactValue,
			description: null,
			effectiveFrom: draft.effectiveFrom,
			effectiveUntil: draft.effectiveUntil || null,
			gradeLevelIds: draft.gradeLevelIds,
			groupId: null,
			hoursPerSemester: null,
			periodsPerWeek: null,
			subjectType: draft.classification,
			termCode: null
		});
		await reloadSelectedHistory();
	}

	async function publish(id: string, rowVersion: number) {
		await publishSubjectVersion(id, { rowVersion });
		await reloadSelectedHistory();
	}

	function gradeLevelsFor(version: SubjectVersion) {
		return gradeLevelOptions.filter((level) => version.gradeLevelIds.includes(level.id));
	}

	onMount(() => loadOverview());
</script>

<PageShell
	title="ทะเบียนรายวิชา"
	description="ดูรหัส ชื่อ ประเภท ระดับชั้น หน่วยกิต และรุ่นที่ใช้อยู่ได้ในหน้าเดียว"
>
	{#if loading}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && subjectItems.length === 0}
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
				<div class="flex flex-col gap-4 border-b bg-muted/25 p-4 lg:flex-row lg:items-end">
					<div class="min-w-0 flex-1">
						<div class="mb-2 flex items-center gap-2 text-sm font-medium">
							<SlidersHorizontal class="size-4 text-primary" /> ค้นหาและกรองรายวิชา
						</div>
						<div class="relative">
							<Search
								class="absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
							/>
							<Label class="sr-only" for="subject-search">ค้นหารายวิชา</Label>
							<Input
								id="subject-search"
								class="ps-9"
								bind:value={search}
								placeholder="ค้นหารหัส ชื่อไทย หรือชื่ออังกฤษ"
							/>
						</div>
					</div>
					<div class="grid gap-3 sm:grid-cols-3 lg:w-[640px]">
						<Select.Root type="single" bind:value={typeFilter}>
							<Select.Trigger aria-label="กรองประเภทรายวิชา" class="w-full">
								{typeFilter === allValue
									? 'ทุกประเภท'
									: optionLabel(SUBJECT_TYPE_OPTIONS, typeFilter)}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value={allValue}>ทุกประเภท</Select.Item>
								{#each SUBJECT_TYPE_OPTIONS as option (option.value)}
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

				{#if canManage}
					<form
						class="flex flex-col gap-3 border-b p-4 sm:flex-row sm:items-end"
						onsubmit={addSubject}
					>
						<div class="min-w-0 flex-1 sm:max-w-xs">
							<Label for="new-subject-code">เพิ่มรหัสรายวิชา</Label>
							<Input
								id="new-subject-code"
								class="mt-1.5 font-mono uppercase"
								bind:value={code}
								placeholder="เช่น ค21101"
								required
							/>
						</div>
						<Button type="submit" disabled={creating}>
							<Plus class="size-4" />
							{creating ? 'กำลังเพิ่ม...' : 'เพิ่มรายวิชา'}
						</Button>
						{#if mutationError}
							<p role="alert" class="text-sm text-destructive">{mutationError}</p>
						{/if}
					</form>
				{/if}

				{#if subjectItems.length === 0}
					<PageState
						class="m-4 border-0 shadow-none"
						title="ยังไม่มีรายวิชาในทะเบียน"
						description="เพิ่มรหัสรายวิชา แล้วสร้างรายละเอียดรุ่นแรกเมื่อพร้อม"
					/>
				{:else if filteredSubjects.length === 0}
					<PageState
						class="m-4 border-0 shadow-none"
						title="ไม่พบรายวิชาที่ตรงกับตัวกรอง"
						description="ลองเปลี่ยนคำค้นหา ประเภท ระดับชั้น หรือสถานะ"
					/>
				{:else}
					<div class="hidden md:block">
						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head class="w-[150px] ps-5">รหัส</Table.Head>
									<Table.Head>ชื่อรายวิชา</Table.Head>
									<Table.Head>ประเภท</Table.Head>
									<Table.Head>ระดับชั้น</Table.Head>
									<Table.Head class="text-end">หน่วยกิต</Table.Head>
									<Table.Head>สถานะ</Table.Head>
									<Table.Head class="w-12"><span class="sr-only">เปิดรายละเอียด</span></Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each filteredSubjects as item (item.subject.id)}
									<Table.Row>
										<Table.Cell class="border-s-4 border-s-primary ps-5 font-mono font-semibold">
											{item.subject.code}
										</Table.Cell>
										<Table.Cell class="max-w-[280px] whitespace-normal">
											<p class="font-medium">
												{item.displayVersion?.nameTh ?? 'ยังไม่มีรายละเอียดรุ่น'}
											</p>
											{#if item.displayVersion?.nameEn}
												<p class="text-xs text-muted-foreground">{item.displayVersion.nameEn}</p>
											{/if}
										</Table.Cell>
										<Table.Cell
											>{optionLabel(
												SUBJECT_TYPE_OPTIONS,
												item.displayVersion?.subjectType
											)}</Table.Cell
										>
										<Table.Cell class="max-w-[220px] whitespace-normal"
											>{gradeLevelSummary(item.gradeLevels)}</Table.Cell
										>
										<Table.Cell class="text-end font-mono tabular-nums"
											>{item.displayVersion?.credit ?? '—'}</Table.Cell
										>
										<Table.Cell>
											<div class="flex flex-wrap gap-1.5">
												<Badge variant="outline" class={displayStateClass(item.displayState)}>
													{displayStateLabel(item.displayState)}
												</Badge>
												{#if item.draftCount > 0}<Badge variant="secondary"
														>ร่าง {item.draftCount}</Badge
													>{/if}
											</div>
										</Table.Cell>
										<Table.Cell>
											<Button
												variant="ghost"
												size="icon"
												aria-label={`เปิด ${item.subject.code}`}
												onclick={() => openSubject(item)}
											>
												<ArrowUpRight class="size-4" />
											</Button>
										</Table.Cell>
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
					</div>

					<div class="grid gap-3 p-4 md:hidden">
						{#each filteredSubjects as item (item.subject.id)}
							<button
								type="button"
								class="rounded-xl border border-s-4 border-s-primary bg-background p-4 text-start shadow-xs transition hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
								onclick={() => openSubject(item)}
							>
								<div class="flex items-start justify-between gap-3">
									<div class="min-w-0">
										<p class="font-mono text-sm font-semibold text-primary">{item.subject.code}</p>
										<h2 class="mt-1 font-medium">
											{item.displayVersion?.nameTh ?? 'ยังไม่มีรายละเอียดรุ่น'}
										</h2>
									</div>
									<ArrowUpRight class="size-4 shrink-0 text-muted-foreground" />
								</div>
								<div class="mt-3 grid grid-cols-2 gap-3 text-sm">
									<div>
										<p class="text-xs text-muted-foreground">ประเภท</p>
										<p>{optionLabel(SUBJECT_TYPE_OPTIONS, item.displayVersion?.subjectType)}</p>
									</div>
									<div>
										<p class="text-xs text-muted-foreground">หน่วยกิต</p>
										<p class="font-mono">{item.displayVersion?.credit ?? '—'}</p>
									</div>
									<div class="col-span-2">
										<p class="text-xs text-muted-foreground">ระดับชั้น</p>
										<p>{gradeLevelSummary(item.gradeLevels)}</p>
									</div>
								</div>
								<div class="mt-3 flex flex-wrap gap-1.5">
									<Badge variant="outline" class={displayStateClass(item.displayState)}
										>{displayStateLabel(item.displayState)}</Badge
									>
									{#if item.draftCount > 0}<Badge variant="secondary">ร่าง {item.draftCount}</Badge
										>{/if}
								</div>
							</button>
						{/each}
					</div>
				{/if}
			</section>

			<p class="text-xs text-muted-foreground">
				แสดง {filteredSubjects.length} จาก {subjectItems.length} รายวิชา · ทะเบียนนี้เป็นข้อมูลกลาง ไม่ผูกกับภาคเรียนบนแถบด้านบน
			</p>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</div>
	{/if}
</PageShell>

<Sheet.Root bind:open={sheetOpen}>
	<Sheet.Content class="w-full overflow-y-auto sm:max-w-5xl">
		<Sheet.Header class="pe-8 text-start">
			<Sheet.Title class="flex items-center gap-2">
				<BookOpen class="size-5 text-primary" />
				<span class="font-mono">{selected?.subject.code ?? 'รายวิชา'}</span>
				{#if selected?.displayVersion}<span class="font-sans"
						>· {selected.displayVersion.nameTh}</span
					>{/if}
			</Sheet.Title>
			<Sheet.Description>
				{#if selected?.displayVersion}
					{formatEffectiveRange(
						selected.displayVersion.effectiveFrom,
						selected.displayVersion.effectiveUntil
					)}
				{:else}
					ยังไม่มีรายละเอียดรุ่น เลือกสร้างรุ่นแรกได้ด้านล่าง
				{/if}
			</Sheet.Description>
		</Sheet.Header>
		{#if historyLoading}
			<PageSkeleton variant="cards" rows={3} />
		{:else if historyError}
			<PageState variant="error" title="โหลดประวัติไม่สำเร็จ" description={historyError} />
		{:else if selected}
			<CatalogVersionHistory
				kind="subject"
				code={selected.subject.code}
				items={versions.map((item) => ({
					id: item.id,
					versionNo: item.versionNo,
					name: item.nameTh,
					secondaryName: item.nameEn,
					exactValue: item.credit,
					effectiveFrom: item.effectiveFrom,
					effectiveUntil: item.effectiveUntil,
					classification: item.subjectType,
					gradeLevels: gradeLevelsFor(item),
					status: item.status,
					rowVersion: item.rowVersion
				}))}
				{gradeLevelOptions}
				{canManage}
				onCreate={addVersion}
				onPublish={publish}
			/>
		{/if}
	</Sheet.Content>
</Sheet.Root>
