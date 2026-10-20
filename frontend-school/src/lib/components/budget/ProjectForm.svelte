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
		oncancel,
		onsaved
	}: {
		workspace?: BudgetProjectWorkspace;
		oncancel: () => void;
		onsaved: (value: BudgetProjectWorkspace) => void;
	} = $props();
	let draft = $state(untrack(() => (workspace ? projectInput(workspace) : newProject())));
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
		errors = projectErrors(draft);
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

<form bind:this={form} onsubmit={save} class="space-y-5" data-testid="budget-project-form">
	{#if loading && !catalogs}<PageSkeleton variant="form" rows={4} />{/if}
	{#if readError}<PageState
			variant="error"
			title="โหลดตัวเลือกไม่สำเร็จ"
			description={readError}
			actionLabel="ลองอีกครั้ง"
			onaction={loadOptions}
		/>{/if}
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
			.filter((year) => year.active || year.id === draft.fiscalYearId)
			.map((year) => ({ value: year.id, label: year.name }))}
		disabled={pending || loading}
		error={errors.fiscalYearId}
	/>
	<Choice
		id="budget-project-organization"
		label="หน่วยงานที่รับผิดชอบ"
		required
		bind:value={draft.organizationUnitId}
		options={organizations.map((org) => ({ value: org.id, label: org.name }))}
		disabled={pending || loading}
		error={errors.organizationUnitId}
	/>
	<fieldset class="grid gap-3 sm:grid-cols-2">
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
	{#if error}<PageState variant="error" title="บันทึกโครงการไม่สำเร็จ" description={error} />{/if}
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={loading || !!readError}
			>บันทึกโครงการ</LoadingButton
		>
	</div>
</form>
