<script lang="ts">
	import { onDestroy, untrack, tick } from 'svelte';
	import {
		getBudgetCatalogs,
		getBudgetProject,
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
	import PlanView from './PlanView.svelte';
	import { ApiClientError } from '#lib/api/client.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	let {
		workspace,
		onsaved,
		onreconciled,
		oncancel
	}: {
		workspace: BudgetProjectWorkspace;
		onsaved: (workspace: BudgetProjectWorkspace) => void;
		onreconciled: (workspace: BudgetProjectWorkspace) => void;
		oncancel: () => void;
	} = $props();
	let activities = $state(untrack(() => activityDrafts(workspace)));
	let expectedRevision = $state(untrack(() => workspace.project.revision)),
		metadata = $state(untrack(() => projectInput(workspace)));
	let conflict = $state(false),
		reloading = $state(false),
		reviewed = $state(false),
		latest = $state<BudgetProjectWorkspace | null>(null),
		conflictReadError = $state('');
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
	async function reloadCurrent() {
		reloading = true;
		conflictReadError = '';
		reviewed = false;
		try {
			const value = await getBudgetProject(workspace.project.id, { signal: controller.signal });
			if (!disposed) {
				latest = value;
				onreconciled(value);
			}
		} catch (cause) {
			if (!disposed) conflictReadError = message(cause);
		} finally {
			if (!disposed) reloading = false;
		}
	}
	function acceptCurrent() {
		if (!latest || !reviewed || !latest.capabilities.canEdit) return;
		expectedRevision = latest.project.revision;
		metadata = projectInput(latest);
		conflict = false;
		latest = null;
		error = '';
		intent.clear();
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending || conflict) return;
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
			if (!disposed) {
				error = message(cause);
				if (cause instanceof ApiClientError && cause.status === 409) {
					conflict = true;
					latest = null;
					reviewed = false;
				}
			}
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form bind:this={form} onsubmit={save} class="space-y-4" data-testid="budget-plan-editor">
	{#if conflict}<section
			class="space-y-3 rounded-xl border bg-card p-3 sm:p-4"
			aria-label="ทบทวนแผนล่าสุด"
		>
			<h2 class="font-semibold">แผนมีการเปลี่ยนแปลง ฉบับร่างของคุณยังอยู่</h2>
			<Button type="button" variant="outline" disabled={reloading} onclick={reloadCurrent}
				>{reloading ? 'กำลังโหลดแผนล่าสุด…' : 'โหลดแผนล่าสุดและเก็บฉบับร่าง'}</Button
			>{#if conflictReadError}<PageState
					variant="error"
					title="โหลดแผนล่าสุดไม่สำเร็จ"
					description={conflictReadError}
				/>{/if}{#if latest}<p class="text-sm">
					แผนล่าสุดฉบับที่ {latest.project.revision}: {latest.project.name}
				</p>
				<PlanView activities={latest.activities} {catalogs} />{#if latest.capabilities.canEdit}<div
						class="flex items-start gap-2"
					>
						<Checkbox id="budget-plan-review-latest" bind:checked={reviewed} /><Label
							for="budget-plan-review-latest"
							>ฉันเปรียบเทียบแผนล่าสุดแล้ว และต้องการบันทึกกิจกรรมในฉบับร่างนี้</Label
						>
					</div>
					<Button type="button" variant="outline" disabled={!reviewed} onclick={acceptCurrent}
						>ใช้แผนล่าสุดเป็นฐานและเก็บฉบับร่าง</Button
					>{:else}<PageState
						variant="permission"
						title="แผนล่าสุดไม่อยู่ในสถานะที่คุณแก้ไขได้"
						description="ตรวจสอบสถานะกับผู้รับผิดชอบก่อนดำเนินการต่อ"
					/>{/if}{/if}
		</section>{/if}
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
		><LoadingButton type="submit" loading={pending} disabled={!catalogs || conflict}
			>บันทึกแผนฉบับร่าง</LoadingButton
		>
	</div>
</form>
