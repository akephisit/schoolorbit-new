<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { ProcessTemplateStep } from '#lib/api/budget.js';
	import {
		lookupStaff,
		lookupOrganizationUnits,
		type StaffLookupItem,
		type OrganizationUnitLookupItem
	} from '#lib/api/lookup.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { message } from '#lib/budget/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import Choice from './Choice.svelte';
	let {
		steps = $bindable([]),
		prefix,
		disabled = false
	}: { steps?: ProcessTemplateStep[]; prefix: string; disabled?: boolean } = $props();
	let staff = $state<StaffLookupItem[]>([]),
		organizations = $state<OrganizationUnitLookupItem[]>([]),
		targets = $state<string[]>([]),
		search = $state(''),
		error = $state(''),
		loading = $state(true);
	const request = new LatestRequest();
	async function load() {
		const ticket = request.begin();
		loading = true;
		error = '';
		try {
			const [users, units] = await Promise.all([
				lookupStaff({ activeOnly: true, search, limit: 100 }, { signal: ticket.signal }),
				lookupOrganizationUnits({ activeOnly: true, search, limit: 100 }, { signal: ticket.signal })
			]);
			if (request.isCurrent(ticket.revision)) {
				staff = [...staff.filter((item) => !users.some((user) => user.id === item.id)), ...users];
				organizations = [
					...organizations.filter((item) => !units.some((unit) => unit.id === item.id)),
					...units
				];
			}
		} catch (cause) {
			if (request.isCurrent(ticket.revision)) error = message(cause);
		} finally {
			if (request.isCurrent(ticket.revision)) loading = false;
		}
	}
	$effect.pre(() => {
		untrack(() => {
			void load();
		});
	});
	onDestroy(() => request.abort());
	const options = $derived([
		...staff.map((user) => ({ value: `user:${user.id}`, label: user.name })),
		...organizations.map((unit) => ({
			value: `organization_unit:${unit.id}`,
			label: `หน่วยงาน: ${unit.name}`
		}))
	]);
	function addRecipient(index: number) {
		const target = targets[index];
		if (!target) return;
		const [kind, id] = target.split(':');
		const recipient =
			kind === 'user'
				? { kind: 'user' as const, userId: id }
				: { kind: 'organization_unit' as const, organizationUnitId: id };
		const current = steps[index].recipients ?? [];
		if (
			!current.some(
				(item) =>
					item.kind === recipient.kind &&
					item.userId === recipient.userId &&
					item.organizationUnitId === recipient.organizationUnitId
			)
		)
			steps[index].recipients = [...current, recipient];
		targets[index] = '';
	}
	function recipientLabel(recipient: NonNullable<ProcessTemplateStep['recipients']>[number]) {
		return recipient.kind === 'user'
			? (staff.find((user) => user.id === recipient.userId)?.name ?? 'บุคลากรที่เลือก')
			: recipient.kind === 'organization_unit'
				? `หน่วยงาน: ${organizations.find((unit) => unit.id === recipient.organizationUnitId)?.name ?? 'หน่วยงานที่เลือก'}`
				: `ตำแหน่ง ${recipient.positionCode ?? ''}`;
	}
</script>

<div class="space-y-4">
	<div class="flex flex-wrap items-end gap-2">
		<div class="min-w-0 flex-1 space-y-2">
			<Label for={`${prefix}-search`}>ค้นหาผู้พิจารณาหรือหน่วยงาน</Label><Input
				id={`${prefix}-search`}
				bind:value={search}
				{disabled}
				onkeydown={(event) => {
					if (event.key === 'Enter') {
						event.preventDefault();
						void load();
					}
				}}
			/>
		</div>
		<Button type="button" variant="outline" disabled={disabled || loading} onclick={load}
			>ค้นหา</Button
		>
	</div>
	{#if error}<PageState
			variant="error"
			title="โหลดผู้พิจารณาไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={load}
		/>{/if}{#each steps as step, index (index)}<section
			class="space-y-3 rounded-lg border bg-card p-3"
		>
			<div class="flex items-end gap-2">
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
			<div class="flex items-end gap-2">
				<div class="min-w-0 flex-1">
					<Choice
						id={`${prefix}-recipient-${index}`}
						label="เพิ่มผู้พิจารณา"
						bind:value={targets[index]}
						{options}
						disabled={disabled || loading}
					/>
				</div>
				<Button
					type="button"
					variant="outline"
					disabled={disabled || !targets[index]}
					onclick={() => addRecipient(index)}>เพิ่ม</Button
				>
			</div>
			<ul class="space-y-1">
				{#each step.recipients ?? [] as recipient, recipientIndex (recipientIndex)}<li
						class="flex items-center justify-between gap-2 text-sm"
					>
						<span>{recipientLabel(recipient)}</span><Button
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
