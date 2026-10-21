<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		getBudgetProcess,
		type BudgetProcessDetail,
		type BudgetActionRequest
	} from '#lib/api/budget.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { message } from '#lib/budget/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { LoadingButton, PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { GitPullRequest } from '@lucide/svelte';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	let {
		source,
		kind,
		id,
		revision,
		canAct,
		onaction
	}: {
		source: Promise<RouteLoadResult<BudgetProcessDetail | null>>;
		kind: 'projects' | 'requests' | 'adjustments';
		id: string;
		revision: number;
		canAct: boolean;
		onaction: (input: BudgetActionRequest) => Promise<void>;
	} = $props();
	const region = new BudgetRegion<BudgetProcessDetail | null>(),
		intent = new BudgetMutationIntent();
	let action = $state<BudgetActionRequest['action'] | null>(null),
		comment = $state(''),
		error = $state(''),
		pending = $state(false),
		disposed = false;
	let expectedRevision = $state(0),
		expectedProcessRevision = $state(0);
	const process = $derived(region.value?.process),
		current = $derived(process?.steps.find((step) => step.stepIndex === process.currentStep));
	const actionLabel = $derived(
		action === 'return_for_revision'
			? 'ส่งกลับแก้ไข'
			: action === 'reject'
				? 'ไม่อนุมัติ'
				: action === 'review'
					? 'ตรวจสอบแล้ว'
					: 'อนุมัติ'
	);
	$effect.pre(() => region.consume(source));
	onDestroy(() => {
		disposed = true;
		region.dispose();
	});
	function open(value: BudgetActionRequest['action']) {
		action = value;
		expectedRevision = revision;
		expectedProcessRevision = process?.revision ?? 0;
		error = '';
		comment = '';
		intent.clear();
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!action || pending) return;
		if (
			(['reject', 'return_for_revision'].includes(action) || current?.definition.requiresComment) &&
			!comment.trim()
		) {
			error = 'กรอกความคิดเห็น / เหตุผล';
			return;
		}
		pending = true;
		error = '';
		try {
			const basis = {
				expectedRevision,
				expectedProcessRevision,
				action,
				comment: comment.trim() || null
			};
			await onaction({ ...basis, requestId: intent.for(basis) });
			if (!disposed) {
				action = null;
				region.retry((signal) => getBudgetProcess(kind, id, { signal }));
			}
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<Card.Root role="region" aria-label="การพิจารณา" aria-busy={region.loading}>
	<Card.Header
		><Card.Title class="flex items-center gap-2"
			><GitPullRequest
				class="size-4 text-muted-foreground"
				aria-hidden="true"
			/>การพิจารณา</Card.Title
		></Card.Header
	><Card.Content class="space-y-4">
		{#if region.error}<PageState
				variant="error"
				title="โหลดขั้นตอนพิจารณาไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => getBudgetProcess(kind, id, { signal }))}
			/>{/if}
		{#if region.loading && !region.value}<PageSkeleton
				variant="form"
				rows={2}
			/>{:else if process}<ol class="divide-y">
				{#each process.steps as step (step.stepIndex)}<li
						class="flex flex-wrap items-start justify-between gap-3 py-3 first:pt-0 last:pb-0 text-sm"
					>
						<div>
							<p class:font-semibold={step.stepIndex === process.currentStep}>
								{step.stepIndex + 1}. {step.definition.title}
							</p>
							<p class="text-muted-foreground">
								{step.recipients.map((recipient) => recipient.displayName).join(', ')}
							</p>
						</div>
						<Badge variant="secondary" class="whitespace-nowrap"
							>{step.completedAt
								? 'ดำเนินการแล้ว'
								: step.stepIndex === process.currentStep
									? 'ขั้นตอนปัจจุบัน'
									: 'รอดำเนินการ'}</Badge
						>
					</li>{/each}
			</ol>
			{#if canAct && process.status === 'active'}<div class="flex flex-wrap gap-2 border-t pt-4">
					<Button onclick={() => open(current?.definition.kind === 'review' ? 'review' : 'approve')}
						>{current?.definition.kind === 'review' ? 'ตรวจสอบแล้ว' : 'อนุมัติ'}</Button
					><Button variant="outline" onclick={() => open('return_for_revision')}
						>ส่งกลับแก้ไข</Button
					><Button variant="destructive" onclick={() => open('reject')}>ไม่อนุมัติ</Button>
				</div>{/if}{:else if !region.error}<p class="text-sm text-muted-foreground">
				ยังไม่ได้ส่งพิจารณา
			</p>{/if}
	</Card.Content></Card.Root
>
<Dialog.Root
	open={action !== null}
	onOpenChange={(open) => {
		if (!open && !pending) action = null;
	}}
	><Dialog.Content
		><Dialog.Header
			><Dialog.Title>{actionLabel}</Dialog.Title><Dialog.Description
				>ยืนยันการพิจารณาฉบับที่ {expectedRevision}</Dialog.Description
			></Dialog.Header
		>
		<form onsubmit={save} class="space-y-6">
			<div class="space-y-2">
				<Label for="budget-action-comment">ความคิดเห็น / เหตุผล</Label><Textarea
					id="budget-action-comment"
					bind:value={comment}
					disabled={pending}
				/>
			</div>
			{#if error}<PageState variant="error" title="ดำเนินการไม่สำเร็จ" description={error} />{/if}
			<div class="flex flex-wrap justify-end gap-2 border-t pt-4">
				<Button type="button" variant="outline" disabled={pending} onclick={() => (action = null)}
					>ยกเลิก</Button
				><LoadingButton type="submit" loading={pending}>ยืนยัน{actionLabel}</LoadingButton>
			</div>
		</form></Dialog.Content
	></Dialog.Root
>
