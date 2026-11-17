<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		saveBudgetYearSettings,
		saveBudgetReceivedFunding,
		type BudgetYearSettings
	} from '#lib/api/budget.js';
	import { lookupOrganizationUnits, type OrganizationUnitLookupItem } from '#lib/api/lookup.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { yearDraft, yearInput, type YearDraft } from '#lib/budget/year-drafts.js';
	import { validDate, message } from '#lib/budget/drafts.js';
	import { money, sumSatang, toSatang } from '#lib/budget/money.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import Choice from './Choice.svelte';
	import RoutingEditor from './RoutingEditor.svelte';
	import { toast } from 'svelte-sonner';
	let {
		value,
		draft = $bindable(),
		onpatch,
		onbusy
	}: {
		value: BudgetYearSettings;
		draft: YearDraft;
		onpatch: (value: BudgetYearSettings) => void;
		onbusy: (busy: boolean) => void;
	} = $props();
	let section = $state('funding'),
		pending = $state<'settings' | 'received' | null>(null),
		error = $state(''),
		organizations = $state<OrganizationUnitLookupItem[]>([]),
		organizationError = $state(''),
		organizationLoading = $state(false);
	$effect(() => onbusy(pending !== null));
	const editable = $derived(value.canManage && !value.fiscalYear.isClosed),
		receivable = $derived(value.canRecordFunding && !value.fiscalYear.isClosed),
		disabled = $derived(!editable || pending !== null);
	const intent = new BudgetMutationIntent(),
		controller = new AbortController();
	let disposed = false;
	$effect(() => {
		if (
			section === 'allocations' &&
			value.canManage &&
			!organizations.length &&
			!organizationLoading &&
			!organizationError
		)
			void loadOrganizations();
	});
	onDestroy(() => {
		disposed = true;
		onbusy(false);
		controller.abort();
	});
	async function loadOrganizations() {
		organizationLoading = true;
		organizationError = '';
		try {
			const rows = await lookupOrganizationUnits({ limit: 500 }, { signal: controller.signal });
			if (!disposed) organizations = rows;
		} catch (cause) {
			if (!disposed) organizationError = message(cause);
		} finally {
			if (!disposed) organizationLoading = false;
		}
	}
	const total = (
		key:
			| 'authorizedSatang'
			| 'allocatedSatang'
			| 'reserveSatang'
			| 'unallocatedSatang'
			| 'receivedSatang'
	) =>
		value.sourceBalances.some((row) => row[key] == null)
			? 'ยังไม่ระบุ'
			: `${money(sumSatang(value.sourceBalances.map((row) => row[key] ?? '0')))} บาท`;
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending || !editable) return;
		error = '';
		try {
			if (
				!draft.name.trim() ||
				!validDate(draft.startDate) ||
				!validDate(draft.endDate) ||
				draft.startDate > draft.endDate
			)
				throw new Error('กรอกชื่อและระยะเวลาปีงบประมาณให้ครบ');
			if ([...draft.funds, ...draft.expenseCategories].some((row) => !row.name.trim()))
				throw new Error('กรอกชื่อแหล่งเงินและหมวดรายจ่ายให้ครบ');
			for (const steps of [draft.planSteps, draft.requestSteps, draft.adjustmentSteps])
				if (
					steps.length &&
					(steps.at(-1)?.kind !== 'approve' ||
						steps.some((step) => !step.title.trim() || !step.recipients?.length))
				)
					throw new Error('แต่ละกระบวนการต้องมีผู้พิจารณาครบและจบด้วยขั้นตอนอนุมัติ');
			const basis = yearInput(draft);
			pending = 'settings';
			const next = await saveBudgetYearSettings(value.fiscalYear.id, {
				...basis,
				requestId: intent.for({ operation: 'settings', id: value.fiscalYear.id, ...basis })
			});
			if (!disposed) {
				draft = yearDraft(next);
				onpatch(next);
				toast.success('บันทึกการตั้งค่าแล้ว');
			}
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = null;
		}
	}
	async function saveReceived() {
		if (pending || !receivable) return;
		error = '';
		try {
			const basis = {
				expectedRevision: draft.expectedRevision,
				funds: value.funds.map((fund) => ({
					fundId: fund.id,
					receivedSatang: draft.received[fund.id]?.trim() ? toSatang(draft.received[fund.id]) : null
				}))
			};
			pending = 'received';
			const next = await saveBudgetReceivedFunding(value.fiscalYear.id, {
				...basis,
				requestId: intent.for({ operation: 'received', id: value.fiscalYear.id, ...basis })
			});
			if (!disposed) {
				draft.expectedRevision = next.fiscalYear.revision;
				onpatch(next);
				toast.success('บันทึกยอดรับเงินจริงแล้ว');
			}
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = null;
		}
	}
</script>

{#if value.fiscalYear.isClosed}<PageState
		title="ปิดปีงบประมาณแล้ว"
		description="ดูข้อมูลของปีนี้ได้ โดยไม่สามารถแก้ไขหรือบันทึกรายการเพิ่มเติม"
	/>{/if}
<form
	onsubmit={(event) => {
		if (section === 'received' || !value.canManage) {
			event.preventDefault();
			void saveReceived();
		} else void save(event);
	}}
	class="space-y-5"
>
	<Tabs.Root bind:value={section}
		><div class="min-w-0 overflow-x-auto">
			<Tabs.List class="h-auto w-max" aria-label="หัวข้อการตั้งค่างบประมาณ">
				{#if value.canManage}
					<Tabs.Trigger value="year">ปีงบประมาณ</Tabs.Trigger>
					<Tabs.Trigger value="funds">แหล่งเงิน</Tabs.Trigger>
					<Tabs.Trigger value="categories">หมวดรายจ่าย</Tabs.Trigger>
					<Tabs.Trigger value="funding">กรอบเงิน</Tabs.Trigger>
					<Tabs.Trigger value="allocations">จัดสรรหน่วยงาน</Tabs.Trigger>
					<Tabs.Trigger value="routing">ขั้นตอนพิจารณา</Tabs.Trigger>
					<Tabs.Trigger value="received">รับเงินจริง</Tabs.Trigger>
				{:else}<Tabs.Trigger value="funding">รับเงินจริง</Tabs.Trigger>{/if}
			</Tabs.List>
		</div>
		<section aria-label="กรอบงบประมาณประจำปี" class="mt-4">
			<details class="rounded-xl border bg-card p-3 sm:p-4">
				<summary class="cursor-pointer text-sm font-medium"
					>ยอดที่บันทึกแล้ว · กรอบรวม {total('authorizedSatang')}</summary
				>
				<div class="mt-4 space-y-4">
					<dl class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
						{#each [{ key: 'authorizedSatang' as const, label: 'กรอบงบประมาณรวม' }, { key: 'allocatedSatang' as const, label: 'จัดสรรให้หน่วยงาน' }, { key: 'reserveSatang' as const, label: 'เงินสำรองส่วนกลาง' }, { key: 'unallocatedSatang' as const, label: 'ยังไม่จัดสรร' }, { key: 'receivedSatang' as const, label: 'รับเงินจริงปีนี้' }] as metric (metric.key)}
							<div class="min-w-0">
								<dt class="text-sm text-muted-foreground">{metric.label}</dt>
								<dd class="mt-1 overflow-x-auto whitespace-nowrap font-semibold tabular-nums">
									{total(metric.key)}
								</dd>
							</div>
						{/each}
					</dl>
					<p class="text-sm text-muted-foreground">
						กรอบงบประมาณ = เงินยกมา + เงินจัดสรรใหม่ที่อนุมัติ ·
						ยอดรับเงินจริงบันทึกแยกจากกรอบงบประมาณ · เงินสำรองแยกจากยอดที่ยังไม่จัดสรร
					</p>
					{#if value.sourceBalances.length}<Card.Root class="gap-0 overflow-hidden py-0"
							><Table.Root class="min-w-[64rem]"
								><Table.Header
									><Table.Row
										><Table.Head>แหล่งเงิน</Table.Head
										>{#each ['เงินยกมา', 'จัดสรรใหม่', 'กรอบรวม', 'จัดสรรหน่วยงาน', 'สำรอง', 'ยังไม่จัดสรร', 'ใช้กรอบแล้ว', 'ยังไม่จัดเข้าโครงการ', 'รับเงินจริง'] as label (label)}<Table.Head
												class="text-right whitespace-nowrap">{label}</Table.Head
											>{/each}</Table.Row
									></Table.Header
								><Table.Body
									>{#each value.sourceBalances as balance (balance.fundId)}<Table.Row
											><Table.Cell
												>{value.funds.find((fund) => fund.id === balance.fundId)?.name ??
													'แหล่งเงินเดิม'}</Table.Cell
											>{#each [balance.carryoverSatang, balance.annualFundingSatang, balance.authorizedSatang, balance.allocatedSatang, balance.reserveSatang, balance.unallocatedSatang, balance.committedSatang, balance.remainingSatang, balance.receivedSatang] as amount, i (i)}<Table.Cell
													class="text-right whitespace-nowrap tabular-nums"
													>{amount == null ? 'ยังไม่ระบุ' : money(amount)}</Table.Cell
												>{/each}</Table.Row
										>{/each}</Table.Body
								></Table.Root
							></Card.Root
						>{/if}
				</div>
			</details>
		</section>
		<Tabs.Content value="year" class="space-y-4 pt-4"
			><div class="grid gap-4 sm:grid-cols-3">
				<div class="space-y-2">
					<Label for="budget-year-name">ชื่อปีงบประมาณ *</Label><Input
						id="budget-year-name"
						bind:value={draft.name}
						{disabled}
					/>
				</div>
				<div class="space-y-2">
					<Label for="budget-year-start">วันที่เริ่มต้น *</Label><DatePicker
						id="budget-year-start"
						ariaLabel="วันที่เริ่มต้นปีงบประมาณ"
						bind:value={draft.startDate}
						disabled={disabled || value.fiscalYear.dateLocked}
					/>
				</div>
				<div class="space-y-2">
					<Label for="budget-year-end">วันที่สิ้นสุด *</Label><DatePicker
						id="budget-year-end"
						ariaLabel="วันที่สิ้นสุดปีงบประมาณ"
						bind:value={draft.endDate}
						disabled={disabled || value.fiscalYear.dateLocked}
					/>
				</div>
			</div>
			{#if value.fiscalYear.dateLocked}<p class="text-sm text-muted-foreground">
					มีโครงการใช้งานปีนี้แล้ว จึงไม่สามารถเปลี่ยนช่วงวันที่ได้
				</p>{/if}</Tabs.Content
		>
		{#if value.canManage}<Tabs.Content value="funding" class="space-y-5 pt-4">
				{#if value.canManage}<section class="space-y-4" aria-label="กรอบเงินรายแหล่ง">
						<h2 class="font-semibold">กรอบเงินรายแหล่ง</h2>
						<p class="text-sm text-muted-foreground">
							กำหนดแหล่งเงินและหมวดรายจ่ายก่อน แล้วระบุเงินยกมา เงินจัดสรรใหม่
							และเงินสำรองของแต่ละแหล่ง
						</p>
						{#if draft.funding === null}<p class="text-sm text-muted-foreground">
								ยังไม่กำหนดกรอบงบประมาณของปีนี้ การไม่ระบุวงเงินต่างจากการกำหนดวงเงินเป็นศูนย์
							</p>
							<Button
								type="button"
								variant="outline"
								{disabled}
								onclick={() =>
									(draft.funding = value.funds.map((fund) => ({
										fundId: fund.id,
										carryover: '0',
										annual: '0',
										reserve: '0'
									})))}>กำหนดกรอบงบประมาณ</Button
							>{:else}{#each draft.funding as fund (fund.fundId)}<Card.Root
									><Card.Content class="space-y-3"
										><h3 class="font-medium">
											{draft.funds.find((item) => item.id === fund.fundId)?.name ?? 'แหล่งเงินเดิม'}
										</h3>
										<div class="grid gap-4 sm:grid-cols-3">
											{#each [{ key: 'carryover' as const, label: 'เงินยกมา' }, { key: 'annual' as const, label: 'เงินจัดสรรใหม่ที่อนุมัติ' }, { key: 'reserve' as const, label: 'เงินสำรองส่วนกลาง' }] as field (field.key)}<div
													class="space-y-2"
												>
													<Label for={`budget-funding-${fund.fundId}-${field.key}`}
														>{field.label} (บาท)</Label
													><Input
														id={`budget-funding-${fund.fundId}-${field.key}`}
														inputmode="decimal"
														bind:value={fund[field.key]}
														{disabled}
													/>
												</div>{/each}
										</div></Card.Content
									></Card.Root
								>{/each}{/if}
						<p class="text-sm text-muted-foreground">
							เมื่อเพิ่มแหล่งเงินใหม่ ให้บันทึกการตั้งค่าก่อน แล้วจึงกรอกวงเงินของแหล่งเงินนั้น
						</p>
					</section>{/if}
			</Tabs.Content>{/if}

		<Tabs.Content value={value.canManage ? 'received' : 'funding'} class="space-y-4 pt-4">
			<p class="text-sm text-muted-foreground">
				บันทึกเงินที่รับจริงของปีนี้แยกจากกรอบที่ได้รับอนุมัติ
				การบันทึกส่วนนี้ไม่เปลี่ยนวงเงินจัดสรร
			</p>
			<section class="space-y-4" aria-label="ยอดรับเงินจริง">
				<h2 class="font-semibold">ยอดรับเงินจริงปีนี้</h2>
				<p class="text-sm text-muted-foreground">
					ระบุยอดสะสมที่ได้รับจริงในปีนี้ เว้นว่างเมื่อยังไม่มีข้อมูล กรอก 0
					เมื่อยืนยันว่าไม่ได้รับเงิน
				</p>
				{#each value.funds as fund (fund.id)}<div
						class="grid items-center gap-2 sm:grid-cols-[1fr_16rem]"
					>
						<Label for={`budget-received-${fund.id}`}>{fund.name} (บาท)</Label><Input
							id={`budget-received-${fund.id}`}
							inputmode="decimal"
							bind:value={draft.received[fund.id]}
							disabled={!receivable || pending !== null}
							placeholder="ยังไม่ระบุ"
						/>
					</div>{/each}{#if receivable}<LoadingButton
						type="submit"
						loading={pending === 'received'}
						disabled={pending !== null}>บันทึกยอดรับเงินจริง</LoadingButton
					>{/if}
			</section>
		</Tabs.Content>
		<Tabs.Content value="allocations" class="space-y-4 pt-4"
			><h2 class="font-semibold">จัดสรรกรอบเงินให้หน่วยงาน</h2>
			<p class="text-sm text-muted-foreground">
				วงเงินรวมที่จัดสรรและเงินสำรองต้องไม่เกินกรอบของแต่ละแหล่งเงิน
				วงเงินหน่วยงานต้องรองรับแผนที่อนุมัติแล้ว
			</p>
			{#if organizationError}<PageState
					variant="error"
					title="โหลดหน่วยงานไม่สำเร็จ"
					description={organizationError}
					actionLabel="ลองอีกครั้ง"
					onaction={loadOrganizations}
				/>{/if}{#each draft.allocations as row, i (i)}<div
					class="grid items-end gap-3 rounded-lg border p-3 sm:grid-cols-[1fr_1fr_10rem_auto]"
				>
					<Choice
						id={`budget-allocation-unit-${i}`}
						label="หน่วยงาน"
						bind:value={row.organizationUnitId}
						options={organizations.map((unit) => ({ value: unit.id, label: unit.name }))}
						disabled={disabled || organizationLoading}
					/><Choice
						id={`budget-allocation-fund-${i}`}
						label="แหล่งเงิน"
						bind:value={row.fundId}
						options={value.funds.map((fund) => ({ value: fund.id, label: fund.name }))}
						{disabled}
					/>
					<div class="space-y-2">
						<Label for={`budget-allocation-amount-${i}`}>วงเงิน (บาท)</Label><Input
							id={`budget-allocation-amount-${i}`}
							bind:value={row.amount}
							inputmode="decimal"
							{disabled}
						/>
					</div>
					<Button
						type="button"
						variant="ghost"
						{disabled}
						onclick={() =>
							(draft.allocations = draft.allocations.filter((_, index) => index !== i))}
						>นำออก</Button
					>
				</div>{/each}<Button
				type="button"
				variant="outline"
				disabled={disabled || draft.funding === null || !value.funds.length}
				onclick={() =>
					(draft.allocations = [
						...draft.allocations,
						{ organizationUnitId: '', fundId: '', amount: '0' }
					])}>เพิ่มการจัดสรร</Button
			>{#if value.unitBalances.length}<Table.Root
					><Table.Header
						><Table.Row
							><Table.Head>หน่วยงาน / แหล่งเงิน</Table.Head><Table.Head class="text-right"
								>จัดสรร</Table.Head
							><Table.Head class="text-right">ใช้กรอบแล้ว</Table.Head><Table.Head class="text-right"
								>เหลือจัดทำแผน</Table.Head
							></Table.Row
						></Table.Header
					><Table.Body
						>{#each value.unitBalances as row (`${row.organizationUnitId}|${row.fundId}`)}<Table.Row
								><Table.Cell
									>{organizations.find((unit) => unit.id === row.organizationUnitId)?.name ??
										'หน่วยงานเดิม'} · {value.funds.find((fund) => fund.id === row.fundId)?.name ??
										'แหล่งเงินเดิม'}</Table.Cell
								><Table.Cell class="text-right">{money(row.allocatedSatang)}</Table.Cell><Table.Cell
									class="text-right">{money(row.committedSatang)}</Table.Cell
								><Table.Cell class="text-right"
									>{value.authorizedFunding == null
										? 'รอจัดสรร'
										: money(row.remainingSatang)}</Table.Cell
								></Table.Row
							>{/each}</Table.Body
					></Table.Root
				>{/if}</Tabs.Content
		>
		{#each [{ key: 'funds' as const, tab: 'funds', label: 'แหล่งเงิน' }, { key: 'expenseCategories' as const, tab: 'categories', label: 'หมวดรายจ่าย' }] as collection (collection.key)}<Tabs.Content
				value={collection.tab}
				class="space-y-4 pt-4"
				><h2 class="font-semibold">{collection.label}ของปี {value.fiscalYear.name}</h2>
				{#each draft[collection.key] as item, i (i)}<Card.Root
						><Card.Content class="grid items-center gap-3 sm:grid-cols-[minmax(0,1fr)_12rem]"
							><div class="space-y-2">
								<Label for={`budget-${collection.key}-${i}`}>ชื่อ{collection.label} *</Label><Input
									id={`budget-${collection.key}-${i}`}
									bind:value={item.name}
									{disabled}
								/>
							</div>
							<div class="flex h-9 items-center gap-2 self-end">
								<Checkbox
									id={`budget-${collection.key}-active-${i}`}
									bind:checked={item.active}
									{disabled}
								/><Label for={`budget-${collection.key}-active-${i}`}>เปิดใช้งาน</Label
								>{#if !item.id}<Button
										type="button"
										variant="ghost"
										{disabled}
										onclick={() =>
											(draft[collection.key] = draft[collection.key].filter(
												(_, index) => index !== i
											))}>นำออก</Button
									>{/if}
							</div></Card.Content
						></Card.Root
					>{/each}<Button
					type="button"
					variant="outline"
					{disabled}
					onclick={() =>
						(draft[collection.key] = [...draft[collection.key], { name: '', active: true }])}
					>เพิ่ม{collection.label}</Button
				></Tabs.Content
			>{/each}
		<Tabs.Content value="routing" class="space-y-6 pt-4">
			{#each [{ key: 'planSteps' as const, title: 'พิจารณาแผนโครงการ' }, { key: 'requestSteps' as const, title: 'พิจารณาคำขอใช้เงิน' }, { key: 'adjustmentSteps' as const, title: 'พิจารณาปรับงบประมาณ' }] as process (process.key)}
				<Card.Root
					><Card.Header><Card.Title><h2>{process.title}</h2></Card.Title></Card.Header><Card.Content
						><RoutingEditor
							prefix={`budget-${value.fiscalYear.id}-${process.key}`}
							bind:steps={draft[process.key]}
							{disabled}
						/></Card.Content
					></Card.Root
				>
			{/each}
		</Tabs.Content>
	</Tabs.Root>
	{#if error}<PageState variant="error" title="บันทึกไม่สำเร็จ" description={error} />{/if}
	{#if editable && section !== 'received'}<div class="flex justify-end border-t pt-4">
			<LoadingButton type="submit" loading={pending === 'settings'} disabled={pending !== null}
				>บันทึกการตั้งค่า</LoadingButton
			>
		</div>{/if}
</form>
