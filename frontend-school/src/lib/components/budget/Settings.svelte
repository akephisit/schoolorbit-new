<script lang="ts">
	import { Settings2 } from '@lucide/svelte';
	import * as Card from '#lib/components/ui/card/index.js';
	import { onDestroy, untrack } from 'svelte';
	import {
		getBudgetSettings,
		saveBudgetSettings,
		type BudgetSettings,
		type SaveBudgetSettingsRequest
	} from '#lib/api/budget.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { message, validDate } from '#lib/budget/drafts.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import RoutingEditor from './RoutingEditor.svelte';
	import { toast } from 'svelte-sonner';
	let { source }: { source: Promise<RouteLoadResult<BudgetSettings | null>> } = $props();
	const region = new BudgetRegion<BudgetSettings | null>(),
		intent = new BudgetMutationIntent();
	let draft = $state<
			| (Omit<SaveBudgetSettingsRequest, 'requestId' | 'expectedRevision'> & {
					expectedRevision: number;
			  })
			| null
		>(null),
		pending = $state(false),
		error = $state(''),
		section = $state('fiscal-years'),
		disposed = false;
	$effect.pre(() => {
		region.consume(source);
	});
	$effect(() => {
		const value = region.value;
		if (value)
			untrack(() => {
				if (!draft)
					draft = {
						expectedRevision: value.revision,
						fiscalYears: structuredClone($state.snapshot(value.fiscalYears)),
						funds: structuredClone($state.snapshot(value.funds)),
						expenseCategories: structuredClone($state.snapshot(value.expenseCategories)),
						planSteps: structuredClone($state.snapshot(value.planSteps)),
						requestSteps: structuredClone($state.snapshot(value.requestSteps)),
						adjustmentSteps: structuredClone($state.snapshot(value.adjustmentSteps))
					};
			});
	});
	onDestroy(() => {
		disposed = true;
		region.dispose();
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!draft || pending) return;
		error = '';
		try {
			if (
				draft.fiscalYears.some(
					(year) =>
						!year.name.trim() ||
						!validDate(year.startDate) ||
						!validDate(year.endDate) ||
						year.startDate > year.endDate
				)
			)
				throw new Error('กรอกชื่อและระยะเวลาปีงบประมาณให้ครบ');
			if ([...draft.funds, ...draft.expenseCategories].some((item) => !item.name.trim()))
				throw new Error('กรอกชื่อแหล่งเงินและหมวดรายจ่ายให้ครบ');
			for (const steps of [draft.planSteps, draft.requestSteps, draft.adjustmentSteps])
				if (
					steps.length > 0 &&
					(steps.at(-1)?.kind !== 'approve' ||
						steps.some((step) => !step.title.trim() || !step.recipients?.length))
				)
					throw new Error('แต่ละกระบวนการต้องมีผู้พิจารณาครบและจบด้วยขั้นตอนอนุมัติ');
			pending = true;
			const value = await saveBudgetSettings({ ...draft, requestId: intent.for(draft) });
			if (!disposed) {
				region.patch(value);
				toast.success('บันทึกการตั้งค่าแล้ว');
				draft = {
					...draft,
					expectedRevision: value.revision,
					fiscalYears: structuredClone($state.snapshot(value.fiscalYears)),
					funds: structuredClone($state.snapshot(value.funds)),
					expenseCategories: structuredClone($state.snapshot(value.expenseCategories))
				};
			}
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<PageShell
	icon={Settings2}
	description="จัดการปีงบประมาณ แหล่งเงิน หมวดรายจ่าย และขั้นตอนพิจารณา"
	backHref="/staff/budget"
	backLabel="กลับภาพรวมงบประมาณ"
	title="ตั้งค่างบประมาณ"
	contentClass="space-y-6"
	>{#if region.error}<PageState
			variant="error"
			title="โหลดการตั้งค่าไม่สำเร็จ"
			description={region.error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => region.retry((signal) => getBudgetSettings({ signal }))}
		/>{/if}{#if !draft && region.loading}<PageSkeleton
			variant="form"
			rows={5}
		/>{/if}{#if draft}<form
			onsubmit={save}
			class="max-w-5xl space-y-6"
			data-testid="budget-settings"
		>
			<Tabs.Root bind:value={section}
				><Tabs.List class="h-auto flex-wrap"
					><Tabs.Trigger value="fiscal-years">ปีงบประมาณ</Tabs.Trigger><Tabs.Trigger value="funds"
						>แหล่งเงิน</Tabs.Trigger
					><Tabs.Trigger value="categories">หมวดรายจ่าย</Tabs.Trigger><Tabs.Trigger value="routing"
						>ขั้นตอนพิจารณา</Tabs.Trigger
					></Tabs.List
				><Tabs.Content value="fiscal-years" class="space-y-4 pt-4"
					><div class="space-y-1">
						<h2 class="font-semibold">ปีงบประมาณ</h2>
						<p class="text-sm text-muted-foreground">
							กำหนดชื่อและช่วงวันที่สำหรับการวางแผนงบประมาณ
						</p>
					</div>
					{#each draft.fiscalYears as year, index (index)}<Card.Root
							><Card.Content class="grid items-end gap-4 sm:grid-cols-2 xl:grid-cols-4">
								<div class="space-y-2">
									<Label for={`budget-year-name-${index}`}>ชื่อปีงบประมาณ *</Label><Input
										id={`budget-year-name-${index}`}
										bind:value={year.name}
										disabled={pending}
									/>
								</div>
								<div class="space-y-2">
									<Label for={`budget-year-start-${index}`}>วันที่เริ่มต้น *</Label><DatePicker
										id={`budget-year-start-${index}`}
										value={year.startDate || undefined}
										onValueChange={(value) => (year.startDate = value ?? '')}
										ariaLabel={`วันที่เริ่มต้นปีงบประมาณ ${index + 1}`}
										disabled={pending}
									/>
								</div>
								<div class="space-y-2">
									<Label for={`budget-year-end-${index}`}>วันที่สิ้นสุด *</Label><DatePicker
										id={`budget-year-end-${index}`}
										value={year.endDate || undefined}
										onValueChange={(value) => (year.endDate = value ?? '')}
										ariaLabel={`วันที่สิ้นสุดปีงบประมาณ ${index + 1}`}
										disabled={pending}
									/>
								</div>
								<div class="flex items-center gap-2">
									<Checkbox
										id={`budget-year-active-${index}`}
										bind:checked={year.active}
										disabled={pending}
									/><Label for={`budget-year-active-${index}`}>เปิดใช้งาน</Label
									>{#if !year.id}<Button
											type="button"
											variant="ghost"
											size="sm"
											disabled={pending}
											onclick={() => {
												if (draft)
													draft.fiscalYears = draft.fiscalYears.filter((_, i) => i !== index);
											}}>นำออก</Button
										>{/if}
								</div>
							</Card.Content></Card.Root
						>{/each}<Button
						type="button"
						variant="outline"
						disabled={pending}
						onclick={() => {
							if (draft)
								draft.fiscalYears = [
									...draft.fiscalYears,
									{ name: '', startDate: '', endDate: '', active: true }
								];
						}}>เพิ่มปีงบประมาณ</Button
					></Tabs.Content
				>{#each [{ key: 'funds' as const, tab: 'funds', label: 'แหล่งเงิน' }, { key: 'expenseCategories' as const, tab: 'categories', label: 'หมวดรายจ่าย' }] as collection (collection.key)}<Tabs.Content
						value={collection.tab}
						class="space-y-4 pt-4"
						><div class="space-y-1">
							<h2 class="font-semibold">{collection.label}</h2>
							<p class="text-sm text-muted-foreground">
								จัดการรายการที่ใช้เลือกในแผนและการใช้เงินของโครงการ
							</p>
						</div>
						{#each draft[collection.key] as item, index (index)}<Card.Root
								><Card.Content class="flex flex-wrap items-end gap-4">
									<div class="min-w-0 flex-1 space-y-2">
										<Label for={`budget-${collection.key}-${index}`}>ชื่อ{collection.label} *</Label
										><Input
											id={`budget-${collection.key}-${index}`}
											bind:value={item.name}
											disabled={pending}
										/>
									</div>
									<div class="flex items-center gap-2">
										<Checkbox
											id={`budget-${collection.key}-active-${index}`}
											bind:checked={item.active}
											disabled={pending}
										/><Label for={`budget-${collection.key}-active-${index}`}>เปิดใช้งาน</Label>
									</div>
									{#if !item.id}<Button
											type="button"
											variant="ghost"
											disabled={pending}
											onclick={() => {
												if (draft)
													draft[collection.key] = draft[collection.key].filter(
														(_, i) => i !== index
													);
											}}>นำออก</Button
										>{/if}
								</Card.Content></Card.Root
							>{/each}<Button
							type="button"
							variant="outline"
							disabled={pending}
							onclick={() => {
								if (draft)
									draft[collection.key] = [...draft[collection.key], { name: '', active: true }];
							}}>เพิ่ม{collection.label}</Button
						></Tabs.Content
					>{/each}<Tabs.Content value="routing" class="space-y-6 pt-4"
					>{#if section === 'routing'}{#each [{ key: 'planSteps' as const, label: 'พิจารณาแผนโครงการ' }, { key: 'requestSteps' as const, label: 'พิจารณาคำขอใช้เงิน' }, { key: 'adjustmentSteps' as const, label: 'พิจารณาปรับงบประมาณ' }] as routing (routing.key)}<Card.Root
								class="gap-4"
							>
								<Card.Header><Card.Title><h2>{routing.label}</h2></Card.Title></Card.Header
								><Card.Content>
									<RoutingEditor
										bind:steps={draft[routing.key]}
										prefix={`budget-${routing.key}`}
										disabled={pending}
									/>
								</Card.Content></Card.Root
							>{/each}{/if}</Tabs.Content
				></Tabs.Root
			>{#if error}<PageState
					variant="error"
					title="บันทึกการตั้งค่าไม่สำเร็จ"
					description={error}
				/>{/if}
			<div class="flex flex-wrap justify-end gap-2 border-t pt-5">
				<Button variant="outline" href="/staff/budget" disabled={pending}>กลับภาพรวม</Button>
				<LoadingButton type="submit" loading={pending}>บันทึกการตั้งค่า</LoadingButton>
			</div>
		</form>{/if}</PageShell
>
