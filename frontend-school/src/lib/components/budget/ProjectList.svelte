<script lang="ts">
	import { FolderOpen, Plus, Search, Settings2 } from '@lucide/svelte';
	import * as Card from '#lib/components/ui/card/index.js';
	import ReadAccessNotice from './ReadAccessNotice.svelte';
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { listBudgetProjects, type BudgetProjectPage } from '#lib/api/budget.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { budgetFilters, destination, dateLabel } from '#lib/budget/presentation.js';
	import { money } from '#lib/budget/money.js';
	import { budgetReadPermissions } from '#lib/budget/access.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { PageState, TableSkeleton } from '#lib/components/app-state/index.js';
	import ProjectForm from './ProjectForm.svelte';
	import Paging from './Paging.svelte';
	import StatusBadge from './StatusBadge.svelte';
	let {
		source,
		query
	}: {
		source: Promise<RouteLoadResult<BudgetProjectPage | null>>;
		query: ReturnType<typeof budgetFilters>;
	} = $props();
	const region = new BudgetRegion<BudgetProjectPage | null>();
	let creating = $state(false),
		search = $state(untrack(() => query.search));
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
	<ReadAccessNotice />
	<Card.Root class="gap-0 py-0"
		><Card.Content class="p-3 sm:p-4">
			<form
				class="flex flex-wrap items-end gap-3"
				onsubmit={(event) => {
					event.preventDefault();
					void goto(destination('/staff/budget/projects', { ...query, search, offset: 0 }));
				}}
			>
				<div class="min-w-0 flex-1 space-y-2">
					<Label for="budget-project-search">ค้นหาโครงการ</Label><Input
						id="budget-project-search"
						bind:value={search}
					/>
				</div>
				<Button type="submit" variant="outline"><Search class="size-4" />ค้นหา</Button>
			</form></Card.Content
		></Card.Root
	>
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
					<Table.Root class="min-w-[48rem]"
						><Table.Header
							><Table.Row
								><Table.Head>โครงการ / หน่วยงาน</Table.Head><Table.Head>ระยะเวลา</Table.Head
								><Table.Head class="whitespace-nowrap text-right">งบประมาณ</Table.Head><Table.Head
									>สถานะ</Table.Head
								><Table.Head>รายละเอียด</Table.Head></Table.Row
							></Table.Header
						><Table.Body
							>{#each region.value.items as project (project.id)}<Table.Row
									><Table.Cell
										><p class="min-w-48 font-medium">{project.name}</p>
										<p class="text-sm text-muted-foreground">
											{project.organizationUnitName}
										</p></Table.Cell
									><Table.Cell class="whitespace-nowrap"
										>{dateLabel(project.startDate)} – {dateLabel(project.endDate)}</Table.Cell
									><Table.Cell class="whitespace-nowrap text-right tabular-nums"
										>{money(project.totalSatang)} บาท</Table.Cell
									><Table.Cell><StatusBadge status={project.status} /></Table.Cell><Table.Cell
										><Button
											size="sm"
											variant="outline"
											href={`/staff/budget/projects/${project.id}?returnTo=${encodeURIComponent(page.url.pathname + page.url.search)}`}
											>เปิดโครงการ</Button
										></Table.Cell
									></Table.Row
								>{/each}</Table.Body
						></Table.Root
					>
				</div></Card.Root
			>{:else if region.value && !region.error}<PageState
				title={query.search ? 'ไม่พบโครงการตามคำค้น' : 'ยังไม่มีโครงการงบประมาณ'}
				description={query.search
					? 'ลองใช้คำค้นอื่นเพื่อค้นหาโครงการในขอบเขตงานของคุณ'
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
				oncancel={() => (creating = false)}
				onsaved={(value) => {
					creating = false;
					void goto(`/staff/budget/projects/${value.project.id}`);
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
