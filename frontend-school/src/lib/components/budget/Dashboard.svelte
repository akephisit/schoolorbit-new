<script lang="ts">
	import { ChartNoAxesCombined, Wallet, Settings2, Download } from '@lucide/svelte';
	import * as Card from '#lib/components/ui/card/index.js';
	import ReadAccessNotice from './ReadAccessNotice.svelte';
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		getBudgetDashboard,
		getBudgetCatalogs,
		exportBudgetReport,
		type BudgetDashboard,
		type BudgetCatalogs,
		type BudgetReportQuery
	} from '#lib/api/budget.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { destination } from '#lib/budget/presentation.js';
	import { money } from '#lib/budget/money.js';
	import { message } from '#lib/budget/drafts.js';
	import { saveDownload } from '#lib/budget/download.js';
	import { can } from '#lib/stores/permissions.js';
	import { budgetExportPermissions, budgetReadPermissions } from '#lib/budget/access.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { PageState, TableSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import Choice from './Choice.svelte';
	import PivotReport from './PivotReport.svelte';
	import type { BudgetPivotMetric } from '#lib/budget/pivot.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	let {
		source,
		catalogsSource,
		query,
		reports = false
	}: {
		source: Promise<RouteLoadResult<BudgetDashboard | null>>;
		catalogsSource: Promise<RouteLoadResult<BudgetCatalogs | null>>;
		query: BudgetReportQuery;
		reports?: boolean;
	} = $props();
	const exportPreview = $derived(reports && $can.hasAny(...budgetExportPermissions));
	const scopedQuery = $derived({ ...query, exportScope: exportPreview || undefined });
	const region = new BudgetRegion<BudgetDashboard | null>(),
		catalogs = new BudgetRegion<BudgetCatalogs | null>();
	let exporting = $state(false),
		exportError = $state('');
	const metric = $derived.by<BudgetPivotMetric>(() => {
		const value = (page.shallow?.url ?? page.url).searchParams.get('metric');
		return value === 'current' || value === 'paid' ? value : 'planned';
	});
	const controller = new AbortController();
	$effect.pre(() => {
		region.consume(source);
		catalogs.consume(catalogsSource);
	});
	onDestroy(() => {
		region.dispose();
		catalogs.dispose();
		controller.abort();
	});
	async function download() {
		exporting = true;
		exportError = '';
		try {
			const blob = await exportBudgetReport({ ...scopedQuery, metric }, controller.signal);
			if (!controller.signal.aborted) saveDownload(blob, 'รายงานงบประมาณ.csv');
		} catch (cause) {
			if (!controller.signal.aborted) exportError = message(cause);
		} finally {
			if (!controller.signal.aborted) exporting = false;
		}
	}
</script>

<PageShell
	icon={reports ? ChartNoAxesCombined : Wallet}
	description={reports
		? 'เปรียบเทียบแผนและการใช้เงินตามกิจกรรม แหล่งเงิน และหมวดรายจ่าย'
		: 'ติดตามงบประมาณที่อนุมัติ ยอดผูกพัน และเงินคงเหลือในขอบเขตงานของคุณ'}
	title={reports ? 'รายงานงบประมาณ' : 'ภาพรวมงบประมาณ'}
	contentClass="space-y-6"
>
	{#snippet actions()}{#if $can.has(PERMISSIONS.BUDGET_MANAGE_SCHOOL)}<Button
				variant="outline"
				href="/staff/budget/settings"><Settings2 class="size-4" />ตั้งค่างบประมาณ</Button
			>{/if}{#if reports && $can.hasAny(PERMISSIONS.BUDGET_DOWNLOAD_OWN, PERMISSIONS.BUDGET_DOWNLOAD_ORGANIZATION_UNIT, PERMISSIONS.BUDGET_DOWNLOAD_ORGANIZATION_TREE, PERMISSIONS.BUDGET_DOWNLOAD_SCHOOL)}<LoadingButton
				loading={exporting}
				disabled={!region.value}
				onclick={download}><Download class="size-4" />ส่งออก CSV</LoadingButton
			>{/if}{/snippet}
	<ReadAccessNotice />
	{#if reports && exportPreview}<p class="text-sm text-muted-foreground">
			รายงานนี้แสดงเฉพาะข้อมูลที่คุณมีสิทธิ์ดูและส่งออก
		</p>{/if}
	<section
		class="flex flex-wrap items-end gap-3 rounded-xl border bg-card p-3 sm:p-4"
		aria-label="เลือกปีงบประมาณ"
	>
		{#if catalogs.error}<PageState
				variant="error"
				title="โหลดปีงบประมาณไม่สำเร็จ"
				description={catalogs.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => catalogs.retry((signal) => getBudgetCatalogs({ signal }))}
			/>{/if}
		<div class="w-full sm:w-64">
			<Choice
				id="budget-dashboard-year"
				label="ปีงบประมาณ"
				value={query.fiscalYearId ?? ''}
				options={[
					{ value: '', label: 'ทุกปีงบประมาณ' },
					...(catalogs.value?.fiscalYears ?? []).map((year) => ({
						value: year.id,
						label: year.name
					}))
				]}
				disabled={catalogs.loading && !catalogs.value}
				onchange={(fiscalYearId) => goto(destination(page.url.pathname, { fiscalYearId }))}
			/>
		</div>
	</section>
	{#if reports}<ToggleGroup.Root
			type="single"
			value={metric}
			aria-label="ยอดที่แสดงในรายงาน"
			onValueChange={(value) => {
				if (value === 'planned' || value === 'current' || value === 'paid')
					void goto(
						destination(page.url.pathname, {
							fiscalYearId: query.fiscalYearId ?? undefined,
							metric: value
						}),
						{ shallow: true }
					);
			}}
			><ToggleGroup.Item value="planned">แผนอนุมัติ</ToggleGroup.Item><ToggleGroup.Item
				value="current">งบปัจจุบัน</ToggleGroup.Item
			><ToggleGroup.Item value="paid">จ่ายจริง</ToggleGroup.Item></ToggleGroup.Root
		>{/if}
	{#if exportError}<PageState
			variant="error"
			title="ส่งออกรายงานไม่สำเร็จ"
			description={exportError}
		/>{/if}
	<section
		class="space-y-4"
		aria-label="สรุปงบประมาณ"
		aria-busy={region.loading}
		data-testid="budget-dashboard"
	>
		{#if region.error}<PageState
				variant="error"
				title="โหลดสรุปงบประมาณไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() =>
					region.retry((signal) => getBudgetDashboard({ signal, query: scopedQuery }))}
			/>{/if}{#if region.loading && !region.value}<TableSkeleton
				columns={6}
			/>{:else if region.value}<div class="grid grid-cols-2 gap-4 xl:grid-cols-4">
				{#each [{ label: 'งบประมาณปัจจุบัน', value: region.value.totals.currentSatang }, { label: 'ผูกพัน', value: region.value.totals.reservedSatang }, { label: 'จ่ายจริง', value: region.value.totals.paidSatang }, { label: 'คงเหลือใช้ได้', value: region.value.totals.availableSatang }] as metric, index (metric.label)}<Card.Root
						class={`min-w-0 gap-0 py-0 ${index === 3 ? 'border-primary/20 bg-primary/5' : ''}`}
						><Card.Content class="min-w-0 p-4"
							><dl>
								<dt class="text-sm text-muted-foreground [overflow-wrap:anywhere]">
									{metric.label}
								</dt>
								<dd class="mt-2 text-xl font-semibold tabular-nums [overflow-wrap:anywhere]">
									{money(metric.value)} <span class="text-sm font-normal">บาท</span>
								</dd>
							</dl>
						</Card.Content></Card.Root
					>{/each}
			</div>
			<p class="text-sm text-muted-foreground tabular-nums">
				แผนอนุมัติ {money(region.value.totals.plannedSatang)} · ปรับงบ {money(
					region.value.totals.adjustedSatang
				)} · เงินยืมค้างคืน {money(region.value.totals.loanOutstandingSatang)} บาท
			</p>
			<div class="space-y-1">
				<h2 class="font-semibold">
					{reports ? 'สรุปตามกิจกรรมและแหล่งเงิน' : 'รายละเอียดงบประมาณ'}
				</h2>
				<p class="text-sm text-muted-foreground">จำนวนเงินเป็นบาท แยกตามแหล่งเงินและหมวดรายจ่าย</p>
			</div>
			{#if region.value.rows.length}{#if reports}<PivotReport
						rows={region.value.rows}
						{metric}
					/>{:else}<Card.Root class="gap-0 overflow-hidden py-0"
						><div class="min-w-0">
							<Table.Root class="min-w-[72rem]"
								><Table.Header
									><Table.Row
										><Table.Head>โครงการ / กิจกรรม</Table.Head><Table.Head
											>แหล่งเงิน / หมวดรายจ่าย</Table.Head
										><Table.Head class="whitespace-nowrap text-right">แผนอนุมัติ</Table.Head
										><Table.Head class="whitespace-nowrap text-right">ปรับงบ</Table.Head><Table.Head
											class="whitespace-nowrap text-right">ปัจจุบัน</Table.Head
										><Table.Head class="whitespace-nowrap text-right">ผูกพัน</Table.Head><Table.Head
											class="whitespace-nowrap text-right">จ่ายจริง</Table.Head
										><Table.Head class="whitespace-nowrap text-right">ใช้ได้</Table.Head><Table.Head
											class="whitespace-nowrap text-right">ยืมค้างคืน</Table.Head
										></Table.Row
									></Table.Header
								><Table.Body
									>{#each region.value.rows as row (`${row.projectId}|${row.activityId}|${row.fundId}|${row.expenseCategoryId}`)}<Table.Row
											><Table.Cell class="min-w-48"
												><Button
													variant="link"
													class="h-auto p-0 whitespace-normal text-left"
													href={`/staff/budget/projects/${row.projectId}`}>{row.projectName}</Button
												>
												<p class="text-sm text-muted-foreground">{row.activityTitle}</p></Table.Cell
											><Table.Cell class="min-w-40"
												>{row.fundName}
												<p class="text-sm text-muted-foreground">
													{row.expenseCategoryName}
												</p></Table.Cell
											>{#each [row.plannedSatang, row.adjustedSatang, row.currentSatang, row.reservedSatang, row.paidSatang, row.availableSatang, row.loanOutstandingSatang] as value, columnIndex (columnIndex)}<Table.Cell
													class="whitespace-nowrap text-right tabular-nums"
													>{money(value)}</Table.Cell
												>{/each}</Table.Row
										>{/each}</Table.Body
								></Table.Root
							>
						</div></Card.Root
					>{/if}{:else}<PageState
					title="ยังไม่มีรายการงบประมาณที่อนุมัติ"
					description="ยอดแผนและการใช้เงินจะแสดงเมื่อโครงการผ่านการอนุมัติ"
					actionLabel={$can.has(PERMISSIONS.BUDGET_CREATE_OWN) &&
					$can.hasAny(...budgetReadPermissions)
						? 'วางแผนโครงการ'
						: undefined}
					href="/staff/budget/projects"
				/>{/if}{/if}
	</section>
</PageShell>
