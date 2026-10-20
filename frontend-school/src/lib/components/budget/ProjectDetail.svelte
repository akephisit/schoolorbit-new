<script lang="ts">
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		getBudgetProject,
		getBudgetCatalogs,
		getBudgetProcess,
		submitBudgetProject,
		amendBudgetProject,
		actOnBudgetProject,
		type BudgetProjectWorkspace,
		type BudgetCatalogs,
		type BudgetProcessDetail,
		type BudgetRequestPage
	} from '#lib/api/budget.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { message } from '#lib/budget/drafts.js';
	import { budgetFilters, dateLabel } from '#lib/budget/presentation.js';
	import { money } from '#lib/budget/money.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import Navigation from './Navigation.svelte';
	import ProjectForm from './ProjectForm.svelte';
	import PlanEditor from './PlanEditor.svelte';
	import PlanView from './PlanView.svelte';
	import RequestForm from './RequestForm.svelte';
	import RequestList from './RequestList.svelte';
	import WorkflowPanel from './WorkflowPanel.svelte';
	import History from './History.svelte';
	import StatusBadge from './StatusBadge.svelte';
	let {
		id,
		source,
		catalogsSource,
		processSource,
		requestsSource,
		query,
		returnTo
	}: {
		id: string;
		source: Promise<RouteLoadResult<BudgetProjectWorkspace | null>>;
		catalogsSource: Promise<RouteLoadResult<BudgetCatalogs | null>>;
		processSource: Promise<RouteLoadResult<BudgetProcessDetail | null>>;
		requestsSource: Promise<RouteLoadResult<BudgetRequestPage | null>>;
		query: ReturnType<typeof budgetFilters>;
		returnTo: string;
	} = $props();
	const region = new BudgetRegion<BudgetProjectWorkspace | null>(),
		catalogs = new BudgetRegion<BudgetCatalogs | null>(),
		intent = new BudgetMutationIntent();
	let editing = $state(false),
		metadataOpen = $state(false),
		requestOpen = $state(false),
		amendmentOpen = $state(false),
		amendmentReason = $state(''),
		error = $state(''),
		pending = $state(false),
		disposed = false;
	let nextProcess = $state<Promise<RouteLoadResult<BudgetProcessDetail | null>> | null>(null);
	$effect.pre(() => {
		region.consume(source);
		catalogs.consume(catalogsSource);
	});
	onDestroy(() => {
		disposed = true;
		region.dispose();
		catalogs.dispose();
	});
	function patch(value: BudgetProjectWorkspace) {
		region.patch(value);
		editing = false;
		metadataOpen = false;
	}
	async function submit() {
		const value = region.value;
		if (!value || pending) return;
		pending = true;
		error = '';
		try {
			const basis = { expectedRevision: value.project.revision };
			const next = await submitBudgetProject(id, {
				...basis,
				requestId: intent.for({ operation: 'submit', ...basis })
			});
			if (!disposed) {
				patch(next);
				nextProcess = captureRouteLoad(getBudgetProcess('projects', id), 'โหลดขั้นตอนไม่สำเร็จ');
			}
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
	async function amend(event: SubmitEvent) {
		event.preventDefault();
		const value = region.value;
		if (!value || pending) return;
		if (!amendmentReason.trim()) {
			error = 'กรอกเหตุผลการแก้ไขแผน';
			return;
		}
		pending = true;
		error = '';
		try {
			const basis = { expectedRevision: value.project.revision, reason: amendmentReason.trim() };
			const next = await amendBudgetProject(id, { ...basis, requestId: intent.for(basis) });
			if (!disposed) {
				patch(next);
				amendmentOpen = false;
				editing = true;
			}
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<PageShell
	title={region.value?.project.name ?? 'รายละเอียดโครงการงบประมาณ'}
	backHref={returnTo}
	backLabel="กลับรายการโครงการ"
	contentClass="space-y-4"
>
	{#snippet actions()}{#if region.value}{@const value =
				region.value}{#if value.capabilities.canEdit && $can.hasAny(PERMISSIONS.BUDGET_UPDATE_OWN, PERMISSIONS.BUDGET_UPDATE_ORGANIZATION_UNIT, PERMISSIONS.BUDGET_UPDATE_ORGANIZATION_TREE, PERMISSIONS.BUDGET_UPDATE_SCHOOL)}<Button
					variant="outline"
					disabled={pending || editing}
					onclick={() => (metadataOpen = true)}>แก้ไขข้อมูลโครงการ</Button
				>{/if}{#if value.capabilities.canAmend}<Button
					variant="outline"
					disabled={pending}
					onclick={() => {
						amendmentOpen = true;
						error = '';
					}}>แก้ไขแผนที่อนุมัติ</Button
				>{/if}{#if value.capabilities.canCreateRequest}<Button
					disabled={pending}
					onclick={() => (requestOpen = true)}>สร้างคำขอใช้เงิน</Button
				>{/if}{/if}{/snippet}
	<Navigation />
	<section
		class="space-y-4"
		aria-label="แผนโครงการ"
		aria-busy={region.loading}
		data-testid="budget-project-detail"
	>
		{#if region.error}<PageState
				variant="error"
				title="โหลดโครงการไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => getBudgetProject(id, { signal }))}
			/>{/if}
		{#if region.loading && !region.value}<PageSkeleton
				variant="form"
				rows={5}
			/>{:else if region.value}{@const value = region.value}
			<div
				class="flex flex-wrap items-start justify-between gap-3 rounded-xl border border-primary/15 bg-gradient-to-r from-primary/5 to-card p-4"
			>
				<div class="space-y-2">
					<StatusBadge status={value.project.status} />
					<p class="text-sm text-muted-foreground">
						{value.project.organizationUnitName} · {dateLabel(value.project.startDate)} – {dateLabel(
							value.project.endDate
						)}
					</p>
				</div>
				<p class="tabular-nums">งบแผน <strong>{money(value.project.totalSatang)} บาท</strong></p>
			</div>
			{#if error && !amendmentOpen}<PageState
					variant="error"
					title="ดำเนินการไม่สำเร็จ"
					description={error}
				/>{/if}
			{#if editing}<PlanEditor
					workspace={value}
					onsaved={patch}
					oncancel={() => (editing = false)}
				/>{:else}<div class="flex flex-wrap items-center justify-between gap-3">
					<h2 class="font-semibold">
						{value.project.status === 'approved' ? 'แผนที่อนุมัติ' : 'แผนฉบับร่าง'}
					</h2>
					<div class="flex flex-wrap gap-2">
						{#if value.capabilities.canEdit}<Button
								variant="outline"
								disabled={pending}
								onclick={() => (editing = true)}>แก้ไขกิจกรรมและค่าใช้จ่าย</Button
							>{/if}{#if value.capabilities.canSubmit}<LoadingButton
								loading={pending}
								onclick={submit}>ส่งแผนพิจารณา</LoadingButton
							>{/if}
					</div>
				</div>
				{#if value.activities.length}<PlanView
						activities={value.activities}
						catalogs={catalogs.value}
					/>{:else}<PageState
						title="ยังไม่มีกิจกรรม"
						description="เพิ่มกิจกรรมและรายการค่าใช้จ่ายในแผนฉบับร่าง"
					/>{/if}{/if}
			{#if catalogs.error}<PageState
					variant="error"
					title="โหลดชื่อแหล่งเงินและหมวดรายจ่ายไม่สำเร็จ"
					description={catalogs.error}
					actionLabel="ลองอีกครั้ง"
					onaction={() => catalogs.retry((signal) => getBudgetCatalogs({ signal }))}
				/>{/if}
			{#if value.acceptedPlan}<details class="space-y-3 rounded-xl border bg-card p-3 sm:p-4">
					<summary class="cursor-pointer font-semibold"
						>แผนที่อนุมัติ รุ่น {value.acceptedPlan.version} · {money(
							value.acceptedPlan.totalSatang
						)} บาท</summary
					>
					<div class="mt-3 space-y-3">
						<p class="text-sm text-muted-foreground">
							อนุมัติเมื่อ {dateLabel(value.acceptedPlan.acceptedAt)}
						</p>
						<PlanView activities={value.acceptedPlan.activities} catalogs={catalogs.value} />
					</div>
				</details>{/if}
			{#if value.balances.length}<section class="space-y-3">
					<h2 class="font-semibold">ยอดงบประมาณตามกิจกรรมและแหล่งเงิน</h2>
					<div class="overflow-x-auto rounded-lg border bg-card">
						<Table.Root
							><Table.Header
								><Table.Row
									><Table.Head>กิจกรรม</Table.Head><Table.Head>แหล่งเงิน / หมวดรายจ่าย</Table.Head
									><Table.Head>งบปัจจุบัน</Table.Head><Table.Head>ผูกพัน</Table.Head><Table.Head
										>จ่ายจริง</Table.Head
									><Table.Head>ใช้ได้</Table.Head><Table.Head>ยืมค้างคืน</Table.Head></Table.Row
								></Table.Header
							><Table.Body
								>{#each value.balances as balance (`${balance.activityId}|${balance.fundId}|${balance.expenseCategoryId}`)}<Table.Row
										><Table.Cell
											>{(value.acceptedPlan?.activities ?? value.activities).find(
												(activity) => activity.id === balance.activityId
											)?.title ?? '—'}</Table.Cell
										><Table.Cell
											>{catalogs.value?.funds.find((item) => item.id === balance.fundId)?.name ??
												'—'}
											<p class="text-sm text-muted-foreground">
												{catalogs.value?.expenseCategories.find(
													(item) => item.id === balance.expenseCategoryId
												)?.name ?? '—'}
											</p></Table.Cell
										>{#each [balance.currentSatang, balance.reservedSatang, balance.paidSatang, balance.availableSatang, balance.loanOutstandingSatang] as amount}<Table.Cell
												class="whitespace-nowrap text-right tabular-nums"
												>{money(amount)}</Table.Cell
											>{/each}</Table.Row
									>{/each}</Table.Body
							></Table.Root
						>
					</div>
				</section>{/if}
		{/if}
	</section>
	{#if region.value}<WorkflowPanel
			source={nextProcess ?? processSource}
			kind="projects"
			{id}
			revision={region.value.project.revision}
			canAct={region.value.capabilities.canAct && $can.has(PERMISSIONS.BUDGET_APPROVE_ASSIGNED)}
			onaction={async (input) => {
				const value = await actOnBudgetProject(id, input);
				if (!disposed) patch(value);
			}}
		/>{/if}
	<RequestList source={requestsSource} {query} embedded />
	<History kind="projects" {id} />
</PageShell>
<Dialog.Root bind:open={metadataOpen}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-xl"
		><Dialog.Header
			><Dialog.Title>แก้ไขข้อมูลโครงการ</Dialog.Title><Dialog.Description
				>ข้อมูลพื้นฐานและระยะเวลาดำเนินการ</Dialog.Description
			></Dialog.Header
		>{#if metadataOpen && region.value}<ProjectForm
				workspace={region.value}
				onsaved={patch}
				oncancel={() => (metadataOpen = false)}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
<Dialog.Root bind:open={requestOpen}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-2xl"
		><Dialog.Header
			><Dialog.Title>สร้างคำขอใช้เงิน</Dialog.Title><Dialog.Description
				>เลือกรายการจากกิจกรรมในแผนที่อนุมัติ</Dialog.Description
			></Dialog.Header
		>{#if requestOpen && region.value}<RequestForm
				workspace={region.value}
				oncancel={() => (requestOpen = false)}
				onsaved={(value) => {
					requestOpen = false;
					void goto(`/staff/budget/requests/${value.request.id}`);
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
<Dialog.Root bind:open={amendmentOpen}
	><Dialog.Content
		><Dialog.Header
			><Dialog.Title>แก้ไขแผนที่อนุมัติ</Dialog.Title><Dialog.Description
				>แผนที่อนุมัติเดิมยังใช้จนกว่าแผนฉบับแก้ไขจะผ่านการอนุมัติ</Dialog.Description
			></Dialog.Header
		>
		<form onsubmit={amend} class="space-y-4">
			<div class="space-y-2">
				<Label for="budget-amend-reason">เหตุผลการแก้ไข *</Label><Textarea
					id="budget-amend-reason"
					bind:value={amendmentReason}
					disabled={pending}
				/>
			</div>
			{#if error}<PageState
					variant="error"
					title="เริ่มแก้ไขแผนไม่สำเร็จ"
					description={error}
				/>{/if}
			<div class="flex justify-end gap-2">
				<Button
					type="button"
					variant="outline"
					disabled={pending}
					onclick={() => (amendmentOpen = false)}>ยกเลิก</Button
				><LoadingButton type="submit" loading={pending}>เริ่มแก้ไขแผน</LoadingButton>
			</div>
		</form></Dialog.Content
	></Dialog.Root
>
