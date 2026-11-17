<script lang="ts">
	import { Settings2 } from '@lucide/svelte';
	import { onDestroy } from 'svelte';
	import { beforeNavigate, goto } from '$app/navigation';
	import {
		getBudgetSettings,
		getBudgetYearSettings,
		createBudgetFiscalYear,
		closeBudgetFiscalYear,
		deleteBudgetFiscalYear,
		type BudgetSettings,
		type BudgetFiscalYear,
		type BudgetYearSettings
	} from '#lib/api/budget.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { yearDraft, type YearDraft } from '#lib/budget/year-drafts.js';
	import { message, validDate } from '#lib/budget/drafts.js';
	import { destination, dateLabel } from '#lib/budget/presentation.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import Choice from './Choice.svelte';
	import BudgetNavigation from './BudgetNavigation.svelte';
	import YearSettings from './YearSettings.svelte';
	let {
		source,
		indexSource,
		selectedId
	}: {
		source: Promise<RouteLoadResult<BudgetYearSettings | null>>;
		indexSource: Promise<RouteLoadResult<BudgetSettings | null>>;
		selectedId: string;
	} = $props();
	const index = new BudgetRegion<BudgetSettings | null>(),
		region = new BudgetRegion<BudgetYearSettings | null>(),
		intent = new BudgetMutationIntent();
	// Unsaved drafts live only in this authenticated workspace, indexed by their fiscal year.
	let drafts = $state<Record<string, YearDraft>>({}),
		yearChanges = $state<Record<string, BudgetFiscalYear | null>>({}),
		creating = $state(false),
		confirm = $state<'close' | 'delete' | null>(null),
		pending = $state(false),
		childPending = $state(false),
		error = $state(''),
		confirmation = $state('');
	let nextYear = $state({ name: '', startDate: '', endDate: '', copyFromFiscalYearId: '' });
	let disposed = false,
		contextGeneration = 0,
		mutationGeneration = 0;
	beforeNavigate(() => {
		contextGeneration++;
	});
	const value = $derived.by(() => {
		const loaded = region.value;
		if (!loaded || (selectedId && loaded.fiscalYear.id !== selectedId)) return null;
		const updated = yearChanges[loaded.fiscalYear.id];
		if (updated === null) return null;
		return updated && updated.revision > loaded.fiscalYear.revision
			? { ...loaded, fiscalYear: updated }
			: loaded;
	});
	// Successful actions reconcile the index without superseding an in-flight route read.
	const years = $derived.by(() => {
		const current = index.value?.fiscalYears ?? [],
			ids = new Set(current.map((year) => year.id));
		return [
			...current.flatMap((year) => {
				const updated = yearChanges[year.id];
				if (updated === null) return [];
				return [updated && updated.revision > year.revision ? updated : year];
			}),
			...Object.values(yearChanges).filter(
				(year): year is BudgetFiscalYear => year !== null && !ids.has(year.id)
			)
		];
	});
	const manage = $derived($can.has(PERMISSIONS.BUDGET_MANAGE_SCHOOL));
	$effect.pre(() => {
		void selectedId;
		contextGeneration++;
		pending = false;
		creating = false;
		error = '';
		index.consume(indexSource);
		region.consume(source);
		confirm = null;
		confirmation = '';
	});
	$effect(() => {
		if (value && !drafts[value.fiscalYear.id])
			drafts[value.fiscalYear.id] = yearDraft(structuredClone($state.snapshot(value)));
	});
	onDestroy(() => {
		disposed = true;
		index.dispose();
		region.dispose();
	});
	function rememberYear(year: BudgetFiscalYear) {
		const previous = yearChanges[year.id];
		if (previous === null) return;
		if (!previous || year.revision > previous.revision) yearChanges[year.id] = year;
	}
	function patch(next: BudgetYearSettings) {
		region.patch(next);
		rememberYear(next.fiscalYear);
	}
	function beginMutation() {
		const context = contextGeneration,
			mutation = ++mutationGeneration;
		pending = true;
		return {
			current: () => !disposed && context === contextGeneration,
			finish: () => {
				if (!disposed && mutation === mutationGeneration) pending = false;
			}
		};
	}
	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		error = '';
		if (
			!nextYear.name.trim() ||
			!validDate(nextYear.startDate) ||
			!validDate(nextYear.endDate) ||
			nextYear.startDate > nextYear.endDate
		) {
			error = 'กรอกชื่อและช่วงวันที่ปีงบประมาณให้ครบ';
			return;
		}
		const mutation = beginMutation();
		try {
			const basis = {
				...nextYear,
				name: nextYear.name.trim(),
				copyFromFiscalYearId: nextYear.copyFromFiscalYearId || null
			};
			const next = await createBudgetFiscalYear({
				...basis,
				requestId: intent.for({ operation: 'create', ...basis })
			});
			if (!disposed) rememberYear(next.fiscalYear);
			if (mutation.current()) {
				region.patch(next);
				creating = false;
				nextYear = { name: '', startDate: '', endDate: '', copyFromFiscalYearId: '' };
				void goto(destination('/staff/budget/settings', { fiscalYearId: next.fiscalYear.id }));
			}
		} catch (cause) {
			if (mutation.current()) error = message(cause);
		} finally {
			mutation.finish();
		}
	}
	async function lifecycle() {
		if (!value || !confirm || pending || confirmation !== value.fiscalYear.name) return;
		const year = value.fiscalYear,
			operation = confirm;
		const mutation = beginMutation();
		error = '';
		try {
			const basis = { expectedRevision: year.revision };
			const input = { ...basis, requestId: intent.for({ operation, id: year.id, ...basis }) };
			if (operation === 'close') {
				const next = await closeBudgetFiscalYear(year.id, input);
				if (!disposed) {
					delete drafts[year.id];
					rememberYear(next.fiscalYear);
				}
				if (mutation.current()) {
					region.patch(next);
				}
			} else {
				await deleteBudgetFiscalYear(year.id, input);
				if (!disposed) {
					delete drafts[year.id];
					yearChanges[year.id] = null;
				}
				if (mutation.current()) {
					region.patch(null);
					void goto('/staff/budget/settings');
				}
			}
			if (mutation.current()) confirm = null;
		} catch (cause) {
			if (mutation.current()) error = message(cause);
		} finally {
			mutation.finish();
		}
	}
</script>

<PageShell
	icon={Settings2}
	title="ตั้งค่างบประมาณ"
	description="เลือกปีงบประมาณเพื่อจัดการกรอบเงิน การจัดสรร และขั้นตอนพิจารณาของปีนั้น"
	backHref="/staff/budget"
	backLabel="กลับภาพรวมงบประมาณ"
	contentClass="space-y-6"
>
	{#snippet actions()}{#if manage}<Button
				onclick={() => {
					creating = true;
					error = '';
				}}>เพิ่มปีงบประมาณ</Button
			>{/if}{/snippet}
	<BudgetNavigation fiscalYearId={selectedId || value?.fiscalYear.id} />
	{#if index.error}<PageState
			variant="error"
			title="โหลดปีงบประมาณไม่สำเร็จ"
			description={index.error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => index.retry((signal) => getBudgetSettings({ signal }))}
		/>{/if}
	<div class="space-y-5" data-testid="budget-settings">
		<div class="flex flex-wrap items-end gap-3 rounded-xl border bg-card p-3 sm:p-4">
			<div class="w-full sm:w-64">
				<Choice
					id="budget-settings-year"
					label="ปีงบประมาณ"
					value={selectedId || value?.fiscalYear.id || ''}
					options={years.map((year) => ({
						value: year.id,
						label: `${year.name}${year.isClosed ? ' · ปิดปีแล้ว' : ''}`
					}))}
					disabled={pending || childPending}
					onchange={(fiscalYearId) => {
						error = '';
						void goto(destination('/staff/budget/settings', { fiscalYearId }));
					}}
				/>
			</div>
			{#if value}<p class="text-sm text-muted-foreground">
					{dateLabel(value.fiscalYear.startDate)} – {dateLabel(value.fiscalYear.endDate)} · {value
						.fiscalYear.isClosed
						? 'ปิดปีแล้ว'
						: 'เปิดดำเนินการ'}
				</p>{/if}
		</div>
		{#if region.error}<PageState
				variant="error"
				title="โหลดการตั้งค่าไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => {
					const id = selectedId || years[0]?.id;
					if (id) region.retry((signal) => getBudgetYearSettings(id, { signal }));
				}}
			/>{/if}
		{#if !value && (region.loading || index.loading)}<PageSkeleton
				variant="form"
				rows={5}
			/>{:else if value && drafts[value.fiscalYear.id]}{#key value.fiscalYear.id}<YearSettings
					{value}
					bind:draft={drafts[value.fiscalYear.id]}
					onpatch={patch}
					onbusy={(busy) => (childPending = busy)}
				/>{/key}{:else if !region.error}<PageState
				title={selectedId ? 'ไม่พบปีงบประมาณที่เลือก' : 'ยังไม่มีปีงบประมาณ'}
				description="เพิ่มปีงบประมาณเพื่อเริ่มตั้งค่าแหล่งเงินและวางแผนโครงการ"
			/>{/if}
		{#if value && manage}<section
				aria-label="ปิดหรือลบปีงบประมาณ"
				class="space-y-3 rounded-xl border bg-card p-4"
			>
				<h2 class="font-semibold">จัดการสถานะปีงบประมาณ</h2>
				<div class="flex flex-wrap gap-2">
					{#if !value.fiscalYear.isClosed}<Button
							variant="outline"
							disabled={pending || childPending || !value.fiscalYear.canClose}
							onclick={() => {
								confirm = 'close';
								confirmation = '';
								error = '';
							}}>ปิดปีงบประมาณ</Button
						>{/if}{#if value.fiscalYear.canDelete}<Button
							variant="destructive"
							disabled={pending || childPending}
							onclick={() => {
								confirm = 'delete';
								confirmation = '';
								error = '';
							}}>ลบปีงบประมาณ</Button
						>{/if}
					{#if !value.fiscalYear.isClosed && !value.fiscalYear.canClose}<p
							class="w-full text-sm text-muted-foreground"
						>
							ปิดปีได้เมื่อไม่มีรายการรอพิจารณา ยอดผูกพัน หรือเงินยืมค้างคืน
						</p>{/if}
				</div>
			</section>{/if}
	</div>
</PageShell>
<Dialog.Root bind:open={creating}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto"
		><Dialog.Header
			><Dialog.Title>เพิ่มปีงบประมาณ</Dialog.Title><Dialog.Description
				>คัดลอกเฉพาะแหล่งเงิน หมวดรายจ่าย และขั้นตอนพิจารณา
				กรอกวงเงินของปีใหม่และยอดรับเงินจริงแยกต่างหาก</Dialog.Description
			></Dialog.Header
		>
		<form onsubmit={create} class="space-y-4">
			<div class="space-y-2">
				<Label for="budget-new-year-name">ชื่อปีงบประมาณ *</Label><Input
					id="budget-new-year-name"
					bind:value={nextYear.name}
					disabled={pending || childPending}
				/>
			</div>
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-2">
					<Label for="budget-new-year-start">วันที่เริ่มต้น *</Label><DatePicker
						id="budget-new-year-start"
						ariaLabel="วันที่เริ่มต้นปีงบประมาณใหม่"
						bind:value={nextYear.startDate}
						disabled={pending || childPending}
					/>
				</div>
				<div class="space-y-2">
					<Label for="budget-new-year-end">วันที่สิ้นสุด *</Label><DatePicker
						id="budget-new-year-end"
						ariaLabel="วันที่สิ้นสุดปีงบประมาณใหม่"
						bind:value={nextYear.endDate}
						disabled={pending || childPending}
					/>
				</div>
			</div>
			<Choice
				id="budget-copy-year"
				label="คัดลอกการตั้งค่าจากปี"
				bind:value={nextYear.copyFromFiscalYearId}
				options={[
					{ value: '', label: 'เริ่มต้นใหม่' },
					...years.map((year) => ({ value: year.id, label: year.name }))
				]}
				disabled={pending || childPending}
			/>{#if error}<PageState
					variant="error"
					title="เพิ่มปีงบประมาณไม่สำเร็จ"
					description={error}
				/>{/if}<LoadingButton type="submit" loading={pending}>เพิ่มปีงบประมาณ</LoadingButton>
		</form></Dialog.Content
	></Dialog.Root
>
<Dialog.Root
	open={confirm !== null}
	onOpenChange={(open) => {
		if (!open && !pending) confirm = null;
	}}
	><Dialog.Content
		><Dialog.Header
			><Dialog.Title
				>{confirm === 'close' ? 'ปิดปีงบประมาณ' : 'ลบปีงบประมาณ'}
				{value?.fiscalYear.name}</Dialog.Title
			><Dialog.Description
				>{#if confirm === 'close'}เมื่อปิดปีแล้ว จะดูข้อมูลได้อย่างเดียว ไม่สามารถแก้ไขแผน การจัดสรร
					การรับเงิน หรือการใช้เงินได้อีก{:else}ลบปีนี้พร้อมการตั้งค่า
					และโครงการที่ยังไม่เคยอนุมัติทั้งหมด ย้อนกลับไม่ได้{/if}</Dialog.Description
			></Dialog.Header
		>{#if value}{#if confirm === 'delete'}<p class="text-sm">
					ขอบเขตที่จะลบ: {value.fiscalYear.deletionImpact.projects} โครงการ · {value.fiscalYear
						.deletionImpact.activities} กิจกรรม · {value.fiscalYear.deletionImpact.planLines} รายการค่าใช้จ่าย
					· {value.fiscalYear.deletionImpact.attempts} ครั้งที่เสนอแผน · {value.fiscalYear
						.deletionImpact.workflowInstances} กระบวนการพิจารณา · {value.fiscalYear.deletionImpact
						.historyEntries} รายการประวัติ รวมงานพิจารณาที่เกี่ยวข้องทั้งหมด
				</p>{/if}<Label for="budget-year-confirm"
				>พิมพ์ชื่อปี {value.fiscalYear.name} เพื่อยืนยัน</Label
			><Input
				id="budget-year-confirm"
				bind:value={confirmation}
				disabled={pending || childPending}
			/>{#if error}<PageState
					variant="error"
					title="ดำเนินการไม่สำเร็จ"
					description={error}
				/>{/if}<Dialog.Footer
				><Button
					variant="outline"
					disabled={pending || childPending}
					onclick={() => (confirm = null)}>ยกเลิก</Button
				><LoadingButton
					variant={confirm === 'delete' ? 'destructive' : 'default'}
					loading={pending}
					disabled={confirmation !== value.fiscalYear.name}
					onclick={lifecycle}
					>{confirm === 'close' ? 'ยืนยันปิดปีงบประมาณ' : 'ยืนยันลบปีงบประมาณถาวร'}</LoadingButton
				></Dialog.Footer
			>{/if}</Dialog.Content
	></Dialog.Root
>
