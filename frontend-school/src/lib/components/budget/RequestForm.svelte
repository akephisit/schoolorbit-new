<script lang="ts">
	import { onDestroy, untrack, tick } from 'svelte';
	import {
		createBudgetRequest,
		saveBudgetRequest,
		type BudgetProjectWorkspace,
		type BudgetSpendingRequestDetail
	} from '#lib/api/budget.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { bahtInput, toSatang, sumSatang, money } from '#lib/budget/money.js';
	import { message } from '#lib/budget/drafts.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import Choice from './Choice.svelte';
	import EvidenceEditor from './EvidenceEditor.svelte';
	let {
		workspace,
		detail,
		oncancel,
		onsaved
	}: {
		workspace: BudgetProjectWorkspace;
		detail?: BudgetSpendingRequestDetail;
		oncancel: () => void;
		onsaved: (value: BudgetSpendingRequestDetail) => void;
	} = $props();
	const activities = untrack(() => workspace.acceptedPlan?.activities ?? []),
		expectedRevision = untrack(() => detail?.request.revision ?? workspace.project.revision);
	let activityId = $state(untrack(() => detail?.request.activityId ?? '')),
		title = $state(untrack(() => detail?.request.title ?? ''));
	let amounts = $state<Record<string, string>>(
		untrack(() =>
			Object.fromEntries(
				(detail?.lines ?? []).map((line) => [line.planLineId, bahtInput(line.amountSatang)])
			)
		)
	);
	let evidenceIds = $state<string[]>(untrack(() => [...(detail?.evidenceFileIds ?? [])])),
		uploading = $state(false),
		pending = $state(false),
		error = $state(''),
		submitted = $state(false);
	let disposed = false,
		form: HTMLFormElement;
	const intent = new BudgetMutationIntent();
	const activity = $derived(activities.find((value) => value.id === activityId));
	const total = $derived.by(() => {
		try {
			return sumSatang(Object.values(amounts).filter(Boolean).map(toSatang));
		} catch {
			return '0';
		}
	});
	onDestroy(() => {
		disposed = true;
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending || uploading) return;
		error = '';
		submitted = true;
		try {
			if (!activityId || !title.trim()) throw new Error('กรอกชื่อคำขอและเลือกกิจกรรม');
			const lines = (activity?.lines ?? [])
				.filter((line) => amounts[line.id] !== undefined)
				.map((line) => ({ planLineId: line.id, amountSatang: toSatang(amounts[line.id]) }));
			if (!lines.length || lines.some((line) => BigInt(line.amountSatang) <= 0n))
				throw new Error('เลือกรายการและกรอกจำนวนเงินมากกว่า 0');
			pending = true;
			const basis = {
				expectedRevision,
				activityId,
				title: title.trim(),
				lines,
				evidenceFileIds: evidenceIds
			};
			const value = detail
				? await saveBudgetRequest(detail.request.id, { ...basis, requestId: intent.for(basis) })
				: await createBudgetRequest(workspace.project.id, {
						...basis,
						requestId: intent.for(basis)
					});
			if (!disposed) onsaved(value);
		} catch (cause) {
			if (!disposed) {
				error = message(cause);
				await tick();
				form.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
			}
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form bind:this={form} onsubmit={save} class="space-y-4" data-testid="budget-request-form">
	<div class="space-y-2">
		<Label for="budget-request-title">ชื่อคำขอใช้เงิน *</Label><Input
			id="budget-request-title"
			bind:value={title}
			disabled={pending}
			aria-invalid={submitted && !title.trim()}
		/>
	</div>
	<Choice
		id="budget-request-activity"
		label="กิจกรรม"
		required
		bind:value={activityId}
		options={activities.map((value) => ({ value: value.id, label: value.title }))}
		disabled={pending || !!detail}
		onchange={() => (amounts = {})}
		error={submitted && !activityId ? 'เลือกกิจกรรม' : ''}
	/>
	{#if activity}<fieldset class="space-y-3">
			<legend class="mb-2 text-sm font-medium">รายการจากแผนที่อนุมัติ *</legend
			>{#each activity.lines as line (line.id)}<div
					class="grid items-end gap-3 rounded-lg border p-3 sm:grid-cols-[1fr_12rem]"
				>
					<div class="flex items-center gap-2">
						<Checkbox
							id={`budget-request-line-${line.id}`}
							checked={amounts[line.id] !== undefined}
							disabled={pending}
							onCheckedChange={(checked) => {
								if (checked) amounts[line.id] = bahtInput(line.totalSatang);
								else {
									const next = { ...amounts };
									delete next[line.id];
									amounts = next;
								}
							}}
						/><Label for={`budget-request-line-${line.id}`}>{line.description}</Label>
					</div>
					{#if amounts[line.id] !== undefined}<div class="space-y-2">
							<Label for={`budget-request-amount-${line.id}`}>จำนวนเงิน (บาท) *</Label><Input
								id={`budget-request-amount-${line.id}`}
								bind:value={amounts[line.id]}
								inputmode="decimal"
								disabled={pending}
							/>
						</div>{/if}
				</div>{/each}
		</fieldset>{/if}
	{#if detail}<EvidenceEditor
			resourceId={detail.request.id}
			bind:ids={evidenceIds}
			bind:busy={uploading}
			disabled={pending}
		/>{/if}
	<p class="text-right tabular-nums">รวมคำขอ <strong>{money(total)} บาท</strong></p>
	{#if error}<PageState variant="error" title="บันทึกคำขอไม่สำเร็จ" description={error} />{/if}
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending || uploading}
			>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={uploading}
			>บันทึกฉบับร่าง</LoadingButton
		>
	</div>
</form>
