<script lang="ts">
	import { onDestroy, untrack, tick } from 'svelte';
	import {
		getBudgetCatalogs,
		saveBudgetProject,
		type BudgetCatalogs,
		type BudgetProjectWorkspace
	} from '#lib/api/budget.js';
	import {
		activityDrafts,
		activityInput,
		projectInput,
		newActivity,
		lineErrors,
		message,
		previewTotal
	} from '#lib/budget/drafts.js';
	import { sumSatang, money } from '#lib/budget/money.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton, PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import ActivityEditor from './ActivityEditor.svelte';
	let {
		workspace,
		onsaved,
		oncancel
	}: {
		workspace: BudgetProjectWorkspace;
		onsaved: (workspace: BudgetProjectWorkspace) => void;
		oncancel: () => void;
	} = $props();
	let activities = $state(untrack(() => activityDrafts(workspace)));
	const expectedRevision = untrack(() => workspace.project.revision),
		metadata = untrack(() => projectInput(workspace));
	let catalogs = $state<BudgetCatalogs | null>(null),
		error = $state(''),
		readError = $state(''),
		pending = $state(false),
		submitted = $state(false);
	let disposed = false,
		form: HTMLFormElement;
	const controller = new AbortController(),
		intent = new BudgetMutationIntent();
	async function load() {
		readError = '';
		try {
			const value = await getBudgetCatalogs({ signal: controller.signal });
			if (!disposed) catalogs = value;
		} catch (cause) {
			if (!disposed) readError = message(cause);
		}
	}
	$effect.pre(() => {
		untrack(() => {
			void load();
		});
	});
	onDestroy(() => {
		disposed = true;
		controller.abort();
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		submitted = true;
		error = '';
		if (
			activities.some(
				(activity) =>
					!activity.title.trim() ||
					activity.lines.some((line) => Object.keys(lineErrors(line)).length)
			)
		) {
			error = 'ตรวจสอบช่องที่ต้องแก้ไขก่อนบันทึก';
			await tick();
			form.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
			return;
		}
		pending = true;
		try {
			const basis = { ...metadata, expectedRevision, activities: activities.map(activityInput) };
			const value = await saveBudgetProject(workspace.project.id, {
				...basis,
				requestId: intent.for(basis)
			});
			if (!disposed) onsaved(value);
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form bind:this={form} onsubmit={save} class="space-y-4" data-testid="budget-plan-editor">
	{#if readError}<PageState
			variant="error"
			title="โหลดแหล่งเงินและหมวดรายจ่ายไม่สำเร็จ"
			description={readError}
			actionLabel="ลองอีกครั้ง"
			onaction={load}
		/>{:else if !catalogs}<PageSkeleton variant="form" rows={5} />{/if}
	{#if catalogs}{#each activities as activity, index (activity.key)}<ActivityEditor
				bind:activity={activities[index]}
				{catalogs}
				{submitted}
				disabled={pending}
				onremove={() => (activities = activities.filter((value) => value.key !== activity.key))}
			/>{/each}
		<div class="flex flex-wrap items-center justify-between gap-3">
			<Button
				type="button"
				variant="outline"
				disabled={pending}
				onclick={() => (activities = [...activities, newActivity()])}>เพิ่มกิจกรรม</Button
			>
			<p class="tabular-nums">
				รวมแผน <strong
					>{money(sumSatang(activities.flatMap((activity) => activity.lines.map(previewTotal))))} บาท</strong
				>
			</p>
		</div>{/if}
	{#if error}<PageState variant="error" title="บันทึกแผนไม่สำเร็จ" description={error} />{/if}
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={!catalogs}
			>บันทึกแผนฉบับร่าง</LoadingButton
		>
	</div>
</form>
