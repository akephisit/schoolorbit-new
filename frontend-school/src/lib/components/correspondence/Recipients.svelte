<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { getCorrespondenceOptions } from '#lib/api/correspondence.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import type { CorrespondenceOptions, ProcessRecipient } from '#lib/api/correspondence.js';
	import Choice from './Choice.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	let {
		recipients = $bindable([]),
		options,
		disabled = false,
		prefix = 'recipient'
	}: {
		recipients?: ProcessRecipient[];
		options: CorrespondenceOptions;
		disabled?: boolean;
		prefix?: string;
	} = $props();
	let results = $state<CorrespondenceOptions | null>(null),
		query = $state(''),
		searchLoading = $state(false),
		searchError = $state('');
	const searchRequest = new LatestRequest();
	onDestroy(() => searchRequest.abort());
	const visible = $derived(results ?? options);
	const known = $derived({
		users: [...options.users, ...(results?.users ?? [])],
		organizations: [...options.organizations, ...(results?.organizations ?? [])],
		positions: [...options.positions, ...(results?.positions ?? [])]
	});
	async function search() {
		const ticket = searchRequest.begin();
		searchLoading = true;
		searchError = '';
		try {
			const value = await getCorrespondenceOptions(query, { signal: ticket.signal });
			if (searchRequest.isCurrent(ticket.revision)) results = value;
		} catch (cause) {
			if (searchRequest.isCurrent(ticket.revision))
				searchError = cause instanceof Error ? cause.message : 'ค้นหาไม่สำเร็จ';
		} finally {
			if (searchRequest.isCurrent(ticket.revision)) searchLoading = false;
		}
	}
	let kind = $state('user'),
		selected = $state('');
	$effect.pre(() => {
		const selectedKind = kind;
		untrack(() => {
			if (selectedKind) selected = '';
		});
	});
	const choices = $derived(
		kind === 'user'
			? visible.users.map((item) => ({ value: item.id, label: item.displayName }))
			: kind === 'organization_unit'
				? visible.organizations.map((item) => ({ value: item.id, label: item.name }))
				: visible.positions.map((item) => ({
						value: `${item.organizationUnitId}:${item.code}`,
						label: `${known.organizations.find((unit) => unit.id === item.organizationUnitId)?.name ?? ''} · ${item.name}`
					}))
	);
	function add() {
		if (!selected) return;
		let recipient: ProcessRecipient;
		if (kind === 'user') recipient = { kind: 'user', userId: selected };
		else if (kind === 'organization_unit')
			recipient = { kind: 'organization_unit', organizationUnitId: selected };
		else {
			const [organizationUnitId, positionCode] = selected.split(':');
			recipient = { kind: 'organization_position', organizationUnitId, positionCode };
		}
		if (!recipients.some((item) => JSON.stringify(item) === JSON.stringify(recipient)))
			recipients = [...recipients, recipient];
		selected = '';
	}
	function name(item: ProcessRecipient): string {
		if (item.kind === 'user')
			return known.users.find((user) => user.id === item.userId)?.displayName ?? 'บุคลากรที่เลือก';
		const unit =
			known.organizations.find((unit) => unit.id === item.organizationUnitId)?.name ??
			'หน่วยงานที่เลือก';
		return item.kind === 'organization_unit'
			? unit
			: `${unit} · ${known.positions.find((position) => position.organizationUnitId === item.organizationUnitId && position.code === item.positionCode)?.name ?? 'ตำแหน่งที่เลือก'}`;
	}
</script>

<div class="space-y-3">
	<details class="text-sm">
		<summary class="cursor-pointer text-muted-foreground">ค้นหาผู้รับเพิ่มเติม</summary>
		<div class="mt-3 flex gap-2 items-end">
			<div class="flex-1 space-y-2">
				<Label for={`${prefix}-search`}>ชื่อบุคลากร / หน่วยงาน</Label><Input
					id={`${prefix}-search`}
					bind:value={query}
					disabled={disabled || searchLoading}
				/>
			</div>
			<LoadingButton
				type="button"
				variant="outline"
				loading={searchLoading}
				{disabled}
				onclick={search}>ค้นหาผู้รับ</LoadingButton
			>
		</div>
	</details>
	{#if searchError}<PageState
			variant="error"
			title={searchError}
			actionLabel="ลองอีกครั้ง"
			onaction={search}
		/>{/if}
	<div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,2fr)_auto] sm:items-end">
		<Choice
			id={`${prefix}-kind`}
			label="กลุ่มผู้รับ"
			bind:value={kind}
			options={[
				{ value: 'user', label: 'บุคลากร' },
				{ value: 'organization_unit', label: 'หน่วยงาน' },
				{ value: 'organization_position', label: 'ตำแหน่งในหน่วยงาน' }
			]}
			{disabled}
		/>
		<Choice
			id={`${prefix}-target`}
			label="ผู้รับ"
			bind:value={selected}
			options={choices}
			searchable
			{disabled}
		/>
		<Button type="button" variant="outline" onclick={add} disabled={disabled || !selected}
			>เพิ่มผู้รับ</Button
		>
	</div>
	<ul class="space-y-2">
		{#each recipients as recipient, index (index)}<li
				class="flex items-center justify-between gap-2 rounded-md border px-3 py-2 text-sm"
			>
				<span class="min-w-0 break-words">{name(recipient)}</span><Button
					type="button"
					variant="ghost"
					size="sm"
					{disabled}
					onclick={() => {
						recipients = recipients.filter((_, current) => current !== index);
					}}
					aria-label={`นำ ${name(recipient)} ออก`}>นำออก</Button
				>
			</li>{/each}
	</ul>
</div>
