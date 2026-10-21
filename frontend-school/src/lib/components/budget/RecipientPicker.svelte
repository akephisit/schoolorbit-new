<script lang="ts">
	import { onDestroy } from 'svelte';
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
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Check, ChevronsUpDown, LoaderCircle } from '@lucide/svelte';
	type Recipient = NonNullable<ProcessTemplateStep['recipients']>[number];
	let {
		id,
		label,
		selectedKeys,
		disabled = false,
		onselect,
		onresolve
	}: {
		id: string;
		label: string;
		selectedKeys: string[];
		disabled?: boolean;
		onselect: (recipient: Recipient, label: string) => void;
		onresolve: (items: { recipient: Recipient; label: string }[]) => void;
	} = $props();
	let open = $state(false),
		search = $state(''),
		loading = $state(false),
		error = $state(''),
		refresh = $state(0);
	let staff = $state<StaffLookupItem[]>([]),
		units = $state<OrganizationUnitLookupItem[]>([]);
	const request = new LatestRequest();
	$effect.pre(() => {
		const context = { open, disabled, search: search.trim(), refresh };
		if (!context.open || context.disabled) {
			request.abort();
			return;
		}
		const ticket = request.begin();
		loading = true;
		error = '';
		staff = [];
		units = [];
		const timer = setTimeout(
			() => {
				void Promise.all([
					lookupStaff(
						{ activeOnly: true, search: context.search || undefined, limit: 50 },
						{ signal: ticket.signal }
					),
					lookupOrganizationUnits(
						{ activeOnly: true, search: context.search || undefined, limit: 50 },
						{ signal: ticket.signal }
					)
				]).then(
					([people, organizations]) => {
						if (!request.isCurrent(ticket.revision)) return;
						staff = people;
						units = organizations;
						onresolve([
							...people.map((person) => ({
								recipient: { kind: 'user' as const, userId: person.id },
								label: person.name
							})),
							...organizations.map((unit) => ({
								recipient: { kind: 'organization_unit' as const, organizationUnitId: unit.id },
								label: `หน่วยงาน: ${unit.name}`
							}))
						]);
						loading = false;
					},
					(cause) => {
						if (!request.isCurrent(ticket.revision)) return;
						error = message(cause);
						loading = false;
					}
				);
			},
			context.search ? 200 : 0
		);
		return () => {
			clearTimeout(timer);
			request.abort();
		};
	});
	onDestroy(() => request.abort());
	function choose(recipient: Recipient, name: string) {
		const key =
			recipient.kind === 'user'
				? `user:${recipient.userId}`
				: `organization_unit:${recipient.organizationUnitId}`;
		if (disabled || selectedKeys.includes(key)) return;
		onselect(recipient, name);
		open = false;
	}
</script>

<div class="min-w-0 space-y-2">
	<Label for={id}>{label}</Label>
	<Popover.Root bind:open onOpenChange={() => (search = '')}>
		<Popover.Trigger
			>{#snippet child({ props })}<Button
					{...props}
					{id}
					type="button"
					variant="outline"
					role="combobox"
					aria-label={label}
					aria-expanded={open}
					{disabled}
					class="w-full justify-between font-normal"
					><span class="min-w-0 truncate">ค้นหาและเลือกบุคลากรหรือหน่วยงาน</span><ChevronsUpDown
						class="size-4 shrink-0 opacity-50"
						aria-hidden="true"
					/></Button
				>{/snippet}</Popover.Trigger
		>
		<Popover.Content class="w-[min(28rem,calc(100vw-2rem))] p-0" align="start" collisionPadding={8}>
			<Command.Root shouldFilter={false} label={`ค้นหา${label}`}>
				<Command.Input
					aria-label={`ค้นหา${label}`}
					placeholder="พิมพ์ชื่อบุคลากรหรือหน่วยงาน"
					bind:value={search}
				/>
				<Command.List class="max-h-[min(18rem,50vh)]" aria-busy={loading}>
					{#if loading}<p
							role="status"
							class="flex items-center gap-2 p-3 text-sm text-muted-foreground"
						>
							<LoaderCircle class="size-4 animate-spin" aria-hidden="true" />กำลังค้นหา…
						</p>
					{:else if error}<PageState
							variant="error"
							title="ค้นหาผู้พิจารณาไม่สำเร็จ"
							description={error}
							actionLabel="ลองอีกครั้ง"
							onaction={() => refresh++}
						/>
					{:else if !staff.length && !units.length}<p
							role="status"
							class="p-3 text-sm text-muted-foreground"
						>
							ไม่พบผู้พิจารณาที่ตรงกับคำค้น
						</p>
					{:else}
						{#if staff.length}<Command.Group heading="บุคลากร"
								>{#each staff as person (person.id)}{@const selected = selectedKeys.includes(
										`user:${person.id}`
									)}<Command.Item
										value={`user:${person.id}`}
										disabled={selected}
										onSelect={() => choose({ kind: 'user', userId: person.id }, person.name)}
										><Check
											class={selected ? 'opacity-100' : 'opacity-0'}
											aria-hidden="true"
										/><span class="min-w-0 break-words whitespace-normal"
											>{person.name}{#if selected}<span class="ml-2 text-xs text-muted-foreground"
													>เลือกแล้ว</span
												>{/if}</span
										></Command.Item
									>{/each}</Command.Group
							>{/if}
						{#if units.length}<Command.Group heading="หน่วยงาน"
								>{#each units as unit (unit.id)}{@const selected = selectedKeys.includes(
										`organization_unit:${unit.id}`
									)}<Command.Item
										value={`organization_unit:${unit.id}`}
										disabled={selected}
										onSelect={() =>
											choose(
												{ kind: 'organization_unit', organizationUnitId: unit.id },
												`หน่วยงาน: ${unit.name}`
											)}
										><Check
											class={selected ? 'opacity-100' : 'opacity-0'}
											aria-hidden="true"
										/><span class="min-w-0 break-words whitespace-normal"
											>หน่วยงาน: {unit.name}{#if selected}<span
													class="ml-2 text-xs text-muted-foreground">เลือกแล้ว</span
												>{/if}</span
										></Command.Item
									>{/each}</Command.Group
							>{/if}
					{/if}
				</Command.List>
			</Command.Root>
			<p class="border-t px-3 py-2 text-xs text-muted-foreground">
				เลือกเพื่อเพิ่มในขั้นตอนนี้ พิมพ์ชื่อเพื่อค้นหาเพิ่มเติม
			</p>
		</Popover.Content>
	</Popover.Root>
</div>
