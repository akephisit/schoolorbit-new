<script lang="ts">
	import { FolderOpen, Plus, Settings2 } from '@lucide/svelte';
	import * as Card from '#lib/components/ui/card/index.js';
	import ReadAccessNotice from './ReadAccessNotice.svelte';
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		listBudgetProjects,
		type BudgetProjectPage,
		type BudgetCatalogs
	} from '#lib/api/budget.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { budgetFilters, dateLabel } from '#lib/budget/presentation.js';
	import { money } from '#lib/budget/money.js';
	import { budgetReadPermissions } from '#lib/budget/access.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { PageState, TableSkeleton } from '#lib/components/app-state/index.js';
	import ProjectDelete from './ProjectDelete.svelte';
	import ProjectForm from './ProjectForm.svelte';
	import Paging from './Paging.svelte';
	import BudgetNavigation from './BudgetNavigation.svelte';
	import ListFilters from './ListFilters.svelte';
	import StatusBadge from './StatusBadge.svelte';
	let {
		source,
		catalogsSource,
		query
	}: {
		source: Promise<RouteLoadResult<BudgetProjectPage | null>>;
		catalogsSource: Promise<RouteLoadResult<BudgetCatalogs | null>>;
		query: ReturnType<typeof budgetFilters>;
	} = $props();
	const region = new BudgetRegion<BudgetProjectPage | null>();
	let creating = $state(false);
	$effect.pre(() => region.consume(source));
	onDestroy(() => region.dispose());
</script>

<PageShell
	icon={FolderOpen}
	description="วางแผนกิจกรรมและค่าใช้จ่าย พร้อมติดตามสถานะการพิจารณาโครงการ"
	title="โครงการงบประมาณ"
	contentClass="space-y-6"
>
	{#snippet actions()}{#if $can.has(PERMISSIONS.BUDGET_MANAGE_SCHOOL)}<Button
				variant="outline"
				href="/staff/budget/settings"><Settings2 class="size-4" />ตั้งค่างบประมาณ</Button
			>{/if}{#if $can.has(PERMISSIONS.BUDGET_CREATE_OWN) && $can.hasAny(...budgetReadPermissions)}<Button
				onclick={() => (creating = true)}><Plus class="size-4" />สร้างโครงการ</Button
			>{/if}{/snippet}
	<BudgetNavigation fiscalYearId={query.fiscalYearId} />
	<ReadAccessNotice />
	{#if $can.hasAny(...budgetReadPermissions)}<ListFilters
			source={catalogsSource}
			{query}
			path="/staff/budget/projects"
			searchable
		/>{/if}
	{#if region.value}<p class="text-sm text-muted-foreground">
			ทั้งหมด {region.value.total} โครงการในขอบเขตที่คุณมีสิทธิ์ดู
		</p>{/if}
	<section aria-label="โครงการงบประมาณ" aria-busy={region.loading} data-testid="budget-projects">
		{#if region.error}<PageState
				variant="error"
				title="โหลดโครงการไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => listBudgetProjects({ signal, query }))}
			/>{/if}
		{#if region.loading && !region.value}<TableSkeleton
				columns={5}
			/>{:else if region.value?.items.length}<Card.Root class="gap-0 overflow-hidden py-0"
				><div class="min-w-0">
					<Table.Root class="block w-full sm:table sm:min-w-[48rem]"
						><Table.Header class="sr-only sm:not-sr-only"
							><Table.Row
								><Table.Head>โครงการ / หน่วยงาน</Table.Head><Table.Head>ระยะเวลา</Table.Head
								><Table.Head class="whitespace-nowrap text-right">งบประมาณ</Table.Head><Table.Head
									>สถานะ</Table.Head
								><Table.Head>รายละเอียด</Table.Head></Table.Row
							></Table.Header
						><Table.Body class="block sm:table-row-group"
							>{#each region.value.items as project (project.id)}<Table.Row
									class="grid grid-cols-2 gap-x-4 gap-y-2 p-4 sm:table-row sm:p-0"
									><Table.Cell class="col-span-2 sm:table-cell"
										><p class="min-w-48 font-medium">{project.name}</p>
										<p class="text-sm text-muted-foreground">
											{project.organizationUnitName}
										</p></Table.Cell
									><Table.Cell class="col-span-2 whitespace-nowrap text-sm sm:text-base"
										>{dateLabel(project.startDate)} – {dateLabel(project.endDate)}</Table.Cell
									><Table.Cell
										class="col-span-2 flex items-center justify-between gap-3 whitespace-nowrap tabular-nums sm:table-cell sm:text-right"
										><span class="text-sm text-muted-foreground sm:hidden">งบประมาณ</span>{money(
											project.totalSatang
										)} บาท</Table.Cell
									><Table.Cell class="self-center"
										><StatusBadge status={project.status} /></Table.Cell
									><Table.Cell class="flex flex-wrap justify-end gap-2 sm:table-cell"
										><Button
											size="sm"
											variant="outline"
											href={`/staff/budget/projects/${project.id}?returnTo=${encodeURIComponent(page.url.pathname + page.url.search)}`}
											>เปิดโครงการ</Button
										>{#if project.acceptedVersion === null}<ProjectDelete
												id={project.id}
												ondeleted={() => {
													if (region.value)
														region.patch({
															...region.value,
															items: region.value.items.filter((item) => item.id !== project.id),
															total: region.value.total - 1
														});
												}}
											/>{/if}</Table.Cell
									></Table.Row
								>{/each}</Table.Body
						></Table.Root
					>
				</div></Card.Root
			>{:else if region.value && !region.error}<PageState
				title={query.search || query.status || query.fiscalYearId
					? 'ไม่พบโครงการตามตัวกรอง'
					: 'ยังไม่มีโครงการงบประมาณ'}
				description={query.search || query.status || query.fiscalYearId
					? 'ลองเปลี่ยนปีงบประมาณ สถานะ หรือคำค้นเพื่อดูรายการอื่น'
					: $can.has(PERMISSIONS.BUDGET_CREATE_OWN)
						? 'สร้างโครงการและเพิ่มกิจกรรมเพื่อเริ่มวางแผนค่าใช้จ่าย'
						: 'โครงการในขอบเขตที่คุณมีสิทธิ์ดูจะแสดงที่นี่'}
			/>{/if}
	</section>
	{#if region.value}<Paging
			path="/staff/budget/projects"
			{query}
			total={region.value.total}
			limit={region.value.limit}
			offset={region.value.offset}
		/>{/if}
</PageShell>
<Dialog.Root bind:open={creating}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-xl"
		><Dialog.Header
			><Dialog.Title>สร้างโครงการ</Dialog.Title><Dialog.Description
				>ข้อมูลโครงการและระยะเวลาดำเนินการ</Dialog.Description
			></Dialog.Header
		>{#if creating}<ProjectForm
			initialFiscalYearId={query.fiscalYearId}
				oncancel={() => (creating = false)}
				onsaved={(value) => {
					creating = false;
					void goto(`/staff/budget/projects/${value.project.id}`);
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
