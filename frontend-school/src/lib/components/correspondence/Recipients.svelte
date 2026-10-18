<script lang="ts">
	import { onDestroy } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { getCorrespondenceOptions } from '#lib/api/correspondence.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import type { CorrespondenceOptions, ProcessRecipient } from '#lib/api/correspondence.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import Choice from './Choice.svelte';
	import {
		recipientKey,
		recipientLabel,
		mergeRecipientLookups
	} from '#lib/correspondence/recipients.js';
	import { Button } from '#lib/components/ui/button/index.js';
	let {
		recipients = $bindable([]),
		options,
		disabled = false,
		prefix = 'recipient',
		directed = false,
		onadd,
		onlookups
	}: {
		recipients?: ProcessRecipient[];
		options: CorrespondenceOptions;
		disabled?: boolean;
		prefix?: string;
		directed?: boolean;
		onadd?: (recipients: ProcessRecipient[]) => void;
		onlookups?: (options: CorrespondenceOptions) => void;
	} = $props();
	let results = $state<CorrespondenceOptions | null>(null),
		query = $state(''),
		searchLoading = $state(false),
		searchError = $state('');
	const searchRequest = new LatestRequest();
	onDestroy(() => searchRequest.abort());
	const visible = $derived(results ?? options);
	let cached = $state<CorrespondenceOptions>({
		users: [],
		organizations: [],
		positions: [],
		templates: [],
		decisionMakers: [],
		reviewers: [],
		routingSettings: [],
		canManageRoutingSettings: false
	});
	const known = $derived(mergeRecipientLookups(options, cached));
	async function search() {
		const ticket = searchRequest.begin();
		searchLoading = true;
		searchError = '';
		try {
			const value = await getCorrespondenceOptions(query, { signal: ticket.signal });
			if (searchRequest.isCurrent(ticket.revision)) {
				results = value;
				cached = mergeRecipientLookups(cached, value);
				onlookups?.(value);
			}
		} catch (cause) {
			if (searchRequest.isCurrent(ticket.revision))
				searchError = cause instanceof Error ? cause.message : 'ค้นหาไม่สำเร็จ';
		} finally {
			if (searchRequest.isCurrent(ticket.revision)) searchLoading = false;
		}
	}
	let kind = $state('user'),
		selected = $state<string[]>([]);
	const choices = $derived(
		kind === 'user'
			? (directed ? visible.reviewers : visible.users).map((item) => ({
					value: item.id,
					label: item.displayName
				}))
			: kind === 'organization_unit'
				? visible.organizations.map((item) => ({ value: item.id, label: item.name }))
				: visible.positions.map((item) => ({
						value: `${item.organizationUnitId}:${item.code}`,
						label: `${known.organizations.find((unit) => unit.id === item.organizationUnitId)?.name ?? ''} · ${item.name}`
					}))
	);
	function recipientFor(value: string): ProcessRecipient {
		if (kind === 'user') return { kind: 'user', userId: value };
		if (kind === 'organization_unit')
			return { kind: 'organization_unit', organizationUnitId: value };
		const position = known.positions.find(
			(item) => `${item.organizationUnitId}:${item.code}` === value
		)!;
		return {
			kind: 'organization_position',
			organizationUnitId: position.organizationUnitId,
			positionCode: position.code
		};
	}
	function add() {
		const additions = selected.map(recipientFor);
		if (onadd) {
			onadd(additions);
			selected = [];
			return;
		}
		const keys = new SvelteSet(recipients.map(recipientKey));
		recipients = [
			...recipients,
			...additions.filter((item) => {
				const key = recipientKey(item);
				if (keys.has(key)) return false;
				keys.add(key);
				return true;
			})
		];
		selected = [];
	}
	function name(item: ProcessRecipient): string {
		return recipientLabel(item, known);
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
					onkeydown={(event) => {
						if (event.key === 'Enter') {
							event.preventDefault();
							void search();
						}
					}}
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
	<div
		class={directed
			? 'grid gap-3'
			: 'grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,2fr)_auto] sm:items-end'}
	>
		<Choice
			id={`${prefix}-kind`}
			label="กลุ่มผู้รับ"
			bind:value={kind}
			onValueChange={() => {
				selected = [];
			}}
			options={[
				{ value: 'user', label: 'บุคลากร' },
				{ value: 'organization_unit', label: 'หน่วยงาน' },
				{ value: 'organization_position', label: 'ตำแหน่งในหน่วยงาน' }
			]}
			{disabled}
		/>
		<fieldset class="min-w-0 space-y-2">
			<legend class="text-sm font-medium">ผู้รับ (เลือกได้หลายรายการ)</legend>
			<div class="max-h-60 space-y-1 overflow-auto rounded-lg border p-2">
				{#each choices as option (option.value)}<div class="flex items-start gap-2 rounded-md p-2">
						<Checkbox
							id={`${prefix}-target-${option.value}`}
							checked={selected.includes(option.value)}
							{disabled}
							onCheckedChange={(value) => {
								selected =
									value === true
										? [...selected.filter((item) => item !== option.value), option.value]
										: selected.filter((item) => item !== option.value);
							}}
						/>
						<Label
							for={`${prefix}-target-${option.value}`}
							class="min-w-0 break-words leading-relaxed">{option.label}</Label
						>
					</div>{/each}
				{#if !choices.length}<p class="p-2 text-sm text-muted-foreground">
						ไม่พบผู้รับ ลองค้นหาเพิ่มเติม
					</p>{/if}
			</div>
		</fieldset>
		<Button type="button" variant="outline" onclick={add} disabled={disabled || !selected.length}
			>เพิ่มผู้รับ{selected.length ? ` (${selected.length})` : ''}</Button
		>
	</div>
	<p class="text-xs text-muted-foreground">
		{#if directed}หน่วยงานหมายถึงผู้รับผิดชอบหน่วยงานปัจจุบัน ตำแหน่งหมายถึงผู้ดำรงตำแหน่งที่เลือก
		{:else}หน่วยงานหมายถึงสมาชิกในหน่วยงาน ตำแหน่งหมายถึงผู้ดำรงตำแหน่งที่เลือก{/if}
		ระบบบันทึกรายชื่อจริงเมื่อส่งหนังสือ
	</p>
	<ul class="space-y-2" aria-label="ผู้รับที่เพิ่มแล้ว">
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
