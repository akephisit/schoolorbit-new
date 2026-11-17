<script lang="ts">
	import { onDestroy, untrack, tick } from 'svelte';
	import {
		createBudgetProject,
		saveBudgetProject,
		getBudgetCatalogs,
		type BudgetCatalogs,
		type BudgetProjectWorkspace
	} from '#lib/api/budget.js';
	import { lookupOrganizationUnits, type OrganizationUnitLookupItem } from '#lib/api/lookup.js';
	import {
		newProject,
		projectInput,
		activityDrafts,
		activityInput,
		projectErrors,
		message
	} from '#lib/budget/drafts.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import { LoadingButton, PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import Choice from './Choice.svelte';
	import FieldError from './FieldError.svelte';
	let {
		workspace,
		initialFiscalYearId = '',
		oncancel,
		onsaved
	}: {
		workspace?: BudgetProjectWorkspace;
		initialFiscalYearId?: string;
		oncancel: () => void;
		onsaved: (value: BudgetProjectWorkspace) => void;
	} = $props();
	let draft = $state(
		untrack(() =>
			workspace ? projectInput(workspace) : { ...newProject(), fiscalYearId: initialFiscalYearId }
		)
	);
	const expectedRevision = untrack(() => workspace?.project.revision ?? 0);
	let catalogs = $state<BudgetCatalogs | null>(null),
		organizations = $state<OrganizationUnitLookupItem[]>([]),
		loading = $state(true),
		readError = $state(''),
		error = $state(''),
		pending = $state(false);
	let errors = $state<ReturnType<typeof projectErrors>>({});
	const intent = new BudgetMutationIntent(),
		controller = new AbortController();
	const selectedYear = $derived(
		catalogs?.fiscalYears.find((year) => year.id === draft.fiscalYearId)
	);
	let disposed = false,
		form: HTMLFormElement;
	async function loadOptions() {
		loading = true;
		readError = '';
		try {
			const [nextCatalogs, nextOrganizations] = await Promise.all([
				getBudgetCatalogs({ signal: controller.signal }),
				lookupOrganizationUnits({ activeOnly: true, limit: 500 }, { signal: controller.signal })
			]);
			if (!disposed) {
				catalogs = nextCatalogs;
				organizations = nextOrganizations;
			}
		} catch (cause) {
			if (!disposed) readError = message(cause);
		} finally {
			if (!disposed) loading = false;
		}
	}
	$effect.pre(() => {
		untrack(() => {
			void loadOptions();
		});
	});
	onDestroy(() => {
		disposed = true;
		controller.abort();
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		errors = projectErrors(
			draft,
			catalogs?.fiscalYears.find((year) => year.id === draft.fiscalYearId)
		);
		error = '';
		if (Object.keys(errors).length) {
			await tick();
			form.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
			return;
		}
		pending = true;
		try {
			const input = { ...draft, name: draft.name.trim() };
			const basis = workspace
				? { ...input, expectedRevision, activities: activityDrafts(workspace).map(activityInput) }
				: input;
			const requestId = intent.for(basis);
			const value = workspace
				? await saveBudgetProject(workspace.project.id, {
						...input,
						expectedRevision,
						activities: activityDrafts(workspace).map(activityInput),
						requestId
					})
				: await createBudgetProject({ ...input, requestId });
			if (!disposed) onsaved(value);
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form
	bind:this={form}
	onsubmit={save}
	class="mx-auto w-full max-w-2xl space-y-6"
	data-testid="budget-project-form"
>
	{#if loading && !catalogs}<PageSkeleton variant="form" rows={4} />{/if}
	{#if readError}<PageState
			variant="error"
			title="โหลดตัวเลือกไม่สำเร็จ"
			description={readError}
			actionLabel="ลองอีกครั้ง"
			onaction={loadOptions}
		/>{/if}
	<section class="space-y-4" aria-labelledby="budget-project-identity">
		<div class="space-y-1">
			<h3 id="budget-project-identity" class="font-semibold">ข้อมูลโครงการ</h3>
			<p class="text-sm text-muted-foreground">ระบุชื่อ ปีงบประมาณ และหน่วยงานเจ้าของโครงการ</p>
		</div>
		<div class="space-y-2">
			<Label for="budget-project-name">ชื่อโครงการ *</Label><Input
				id="budget-project-name"
				bind:value={draft.name}
				disabled={pending}
				aria-invalid={!!errors.name}
				aria-describedby="budget-project-name-error"
			/><FieldError id="budget-project-name-error" message={errors.name} />
		</div>
		<Choice
			id="budget-project-year"
			label="ปีงบประมาณ"
			required
			bind:value={draft.fiscalYearId}
			options={(catalogs?.fiscalYears ?? [])
				.filter((year) => !year.isClosed || year.id === draft.fiscalYearId)
				.map((year) => ({ value: year.id, label: year.name }))}
			disabled={pending || loading || !!workspace?.acceptedPlan}
			error={errors.fiscalYearId}
		/>
		{#if !workspace && catalogs && draft.fiscalYearId && (!selectedYear || selectedYear.isClosed)}<p
				class="text-sm text-warning"
				role="status"
			>
				{selectedYear?.isClosed
					? 'ปีงบประมาณที่เลือกปิดแล้ว เลือกปีที่เปิดใช้งานเพื่อสร้างโครงการ'
					: 'ไม่พบปีงบประมาณที่เลือก กรุณาเลือกปีงบประมาณอีกครั้ง'}
			</p>{/if}
		<Choice
			id="budget-project-organization"
			label="หน่วยงานที่รับผิดชอบ"
			required
			bind:value={draft.organizationUnitId}
			options={[
				...organizations.map((org) => ({ value: org.id, label: org.name })),
				...(workspace && !organizations.some((org) => org.id === draft.organizationUnitId)
					? [{ value: draft.organizationUnitId, label: workspace.project.organizationUnitName }]
					: [])
			]}
			disabled={pending || loading || !!workspace?.acceptedPlan}
			error={errors.organizationUnitId}
		/>
		{#if workspace?.acceptedPlan}<p class="text-sm text-muted-foreground">
				โครงการมีแผนที่อนุมัติแล้ว จึงคงปีงบประมาณและหน่วยงานเดิมไว้
			</p>{/if}
	</section>
	<section class="space-y-4 border-t pt-6" aria-labelledby="budget-project-period">
		<div class="space-y-1">
			<h3 id="budget-project-period" class="font-semibold">ระยะเวลาดำเนินการ</h3>
			<p class="text-sm text-muted-foreground">กำหนดช่วงเวลาของโครงการเพื่อใช้วางแผนกิจกรรม</p>
		</div>
		<fieldset class="grid gap-5 sm:grid-cols-2">
			<legend class="mb-2 text-sm font-medium">ระยะเวลาดำเนินการ *</legend>
			<div class="space-y-2">
				<Label for="budget-project-start">วันที่เริ่มต้น *</Label><DatePicker
					id="budget-project-start"
					value={draft.startDate || undefined}
					onValueChange={(value) => (draft.startDate = value ?? '')}
					ariaLabel="วันที่เริ่มต้น"
					disabled={pending}
				/><FieldError id="budget-project-start-error" message={errors.startDate} />
			</div>
			<div class="space-y-2">
				<Label for="budget-project-end">วันที่สิ้นสุด *</Label><DatePicker
					id="budget-project-end"
					value={draft.endDate || undefined}
					onValueChange={(value) => (draft.endDate = value ?? '')}
					ariaLabel="วันที่สิ้นสุด"
					disabled={pending}
				/><FieldError id="budget-project-end-error" message={errors.endDate} />
			</div>
		</fieldset>
	</section>
	{#if !workspace}<p class="text-sm text-muted-foreground">
			หลังบันทึกโครงการ เพิ่มกิจกรรมและรายการค่าใช้จ่ายในแผนฉบับร่างได้
		</p>{/if}
	{#if error}<PageState variant="error" title="บันทึกโครงการไม่สำเร็จ" description={error} />{/if}
	<div class="flex flex-wrap justify-end gap-2 border-t pt-5">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={loading || !!readError}
			>บันทึกโครงการ</LoadingButton
		>
	</div>
</form>
