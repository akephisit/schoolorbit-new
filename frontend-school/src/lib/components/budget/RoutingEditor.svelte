<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { ProcessTemplateStep } from '#lib/api/budget.js';
	import { lookupStaff, lookupOrganizationUnits } from '#lib/api/lookup.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { message } from '#lib/budget/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import Choice from './Choice.svelte';
	import RecipientPicker from './RecipientPicker.svelte';
	type Recipient = NonNullable<ProcessTemplateStep['recipients']>[number];
	let {
		steps = $bindable([]),
		prefix,
		disabled = false
	}: { steps?: ProcessTemplateStep[]; prefix: string; disabled?: boolean } = $props();
	let labels = $state<Record<string, string>>({}),
		labelError = $state('');
	const request = new LatestRequest();
	const initialRecipients = untrack(() => steps.flatMap((step) => step.recipients ?? []));
	function recipientKey(recipient: Recipient) {
		return recipient.kind === 'user'
			? `user:${recipient.userId}`
			: recipient.kind === 'organization_unit'
				? `organization_unit:${recipient.organizationUnitId}`
				: `organization_position:${recipient.organizationUnitId}:${recipient.positionCode}`;
	}
	const currentKeys = $derived(steps.flatMap((step) => (step.recipients ?? []).map(recipientKey)));
	$effect(() => {
		const keys = currentKeys;
		untrack(() => {
			labels = Object.fromEntries(Object.entries(labels).filter(([key]) => keys.includes(key)));
		});
	});
	async function loadSelectedLabels() {
		if (!initialRecipients.length) return;
		const ticket = request.begin();
		labelError = '';
		try {
			const [people, units] = await Promise.all([
				initialRecipients.some((item) => item.kind === 'user')
					? lookupStaff({ activeOnly: true, limit: 100 }, { signal: ticket.signal })
					: Promise.resolve([]),
				initialRecipients.some((item) => item.kind === 'organization_unit')
					? lookupOrganizationUnits({ activeOnly: true, limit: 100 }, { signal: ticket.signal })
					: Promise.resolve([])
			]);
			if (!request.isCurrent(ticket.revision)) return;
			const resolved = [
				...people.map((person) => [`user:${person.id}`, person.name]),
				...units.map((unit) => [`organization_unit:${unit.id}`, `หน่วยงาน: ${unit.name}`])
			];
			labels = {
				...Object.fromEntries(resolved.filter(([key]) => currentKeys.includes(key))),
				...labels
			};
		} catch (cause) {
			if (request.isCurrent(ticket.revision)) labelError = message(cause);
		}
	}
	$effect.pre(() => {
		untrack(() => {
			void loadSelectedLabels();
		});
	});
	onDestroy(() => request.abort());
	function addRecipient(step: ProcessTemplateStep, recipient: Recipient, name: string) {
		if (
			disabled ||
			(step.recipients ?? []).some((item) => recipientKey(item) === recipientKey(recipient))
		)
			return;
		labels[recipientKey(recipient)] = name;
		step.recipients = [...(step.recipients ?? []), recipient];
	}
	function recipientLabel(recipient: Recipient, index: number) {
		return (
			labels[recipientKey(recipient)] ??
			(recipient.kind === 'organization_position'
				? `ตำแหน่ง ${recipient.positionCode ?? ''}`
				: `${recipient.kind === 'user' ? 'บุคลากร' : 'หน่วยงาน'}เดิมรายการที่ ${index + 1} (ยังไม่พบชื่อ)`)
		);
	}
</script>

<div class="space-y-4">
	{#if labelError}<PageState
			variant="error"
			title="โหลดชื่อผู้พิจารณาเดิมไม่สำเร็จ"
			description={labelError}
			actionLabel="ลองอีกครั้ง"
			onaction={loadSelectedLabels}
		/>{/if}{#each steps as step, index (step)}<section
			class="space-y-4 rounded-lg border bg-muted/20 p-4"
			aria-label={`ขั้นตอนที่ ${index + 1}`}
		>
			<div class="flex flex-wrap items-end gap-3">
				<div class="min-w-0 flex-1 space-y-2">
					<Label for={`${prefix}-title-${index}`}>ชื่อขั้นตอนที่ {index + 1} *</Label><Input
						id={`${prefix}-title-${index}`}
						bind:value={step.title}
						{disabled}
					/>
				</div>
				<Button
					type="button"
					variant="outline"
					{disabled}
					onclick={() => (steps = steps.filter((_, i) => i !== index))}>ลบขั้นตอน</Button
				>
			</div>
			<div class="grid gap-3 sm:grid-cols-2">
				<Choice
					id={`${prefix}-kind-${index}`}
					label="การดำเนินการ"
					value={step.kind}
					options={[
						{ value: 'review', label: 'ตรวจสอบ' },
						{ value: 'approve', label: 'อนุมัติ' }
					]}
					onchange={(value) => (step.kind = value === 'review' ? 'review' : 'approve')}
					{disabled}
				/><Choice
					id={`${prefix}-policy-${index}`}
					label="เงื่อนไขผู้พิจารณา"
					value={step.completionPolicy}
					options={[
						{ value: 'all', label: 'ทุกคนดำเนินการ' },
						{ value: 'any', label: 'คนใดคนหนึ่งดำเนินการ' }
					]}
					onchange={(value) => (step.completionPolicy = value === 'all' ? 'all' : 'any')}
					{disabled}
				/>
			</div>
			<RecipientPicker
				id={`${prefix}-recipient-${index}`}
				label={`เพิ่มผู้พิจารณาขั้นตอนที่ ${index + 1}`}
				selectedKeys={(step.recipients ?? []).map(recipientKey)}
				{disabled}
				onselect={(recipient, name) => addRecipient(step, recipient, name)}
				onresolve={(items) => {
					labels = {
						...labels,
						...Object.fromEntries(
							items
								.filter((item) => currentKeys.includes(recipientKey(item.recipient)))
								.map((item) => [recipientKey(item.recipient), item.label])
						)
					};
				}}
			/>
			<ul class="space-y-2">
				{#each step.recipients ?? [] as recipient, recipientIndex (recipientIndex)}<li
						class="flex items-center justify-between gap-3 text-sm"
					>
						<span class="min-w-0 break-words">{recipientLabel(recipient, recipientIndex)}</span
						><Button
							type="button"
							variant="ghost"
							size="sm"
							{disabled}
							onclick={() =>
								(step.recipients = (step.recipients ?? []).filter((_, i) => i !== recipientIndex))}
							>นำออก</Button
						>
					</li>{/each}
			</ul>
			<div class="flex items-center gap-2">
				<Checkbox
					id={`${prefix}-comment-${index}`}
					checked={step.requiresComment ?? false}
					onCheckedChange={(checked) => (step.requiresComment = !!checked)}
					{disabled}
				/><Label for={`${prefix}-comment-${index}`}>ต้องระบุความคิดเห็น</Label>
			</div>
		</section>{/each}<Button
		type="button"
		variant="outline"
		{disabled}
		onclick={() =>
			(steps = [
				...steps,
				{
					title: '',
					kind: 'approve',
					completionPolicy: 'all',
					recipients: [],
					requiresComment: false,
					requiresSignature: false,
					requiresEvidence: false,
					requiresDelegatedAuthority: false,
					dueAfterDays: null
				}
			])}>เพิ่มขั้นตอน</Button
	>
</div>
