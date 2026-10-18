<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		listLeaveTypes,
		listLeavePolicies,
		saveLeaveType,
		deleteLeaveType,
		type LeaveType,
		type LeavePolicyVersion
	} from '#lib/api/staff-leave.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import { leaveDate, leaveKindLabels, leaveAmount } from '#lib/staff-leave/presentation.js';
	import { PERSONNEL_TYPE_LABELS } from '#lib/forms/staff-career.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import {
		PageState,
		PageSkeleton,
		TableSkeleton,
		LoadingButton
	} from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import { ChevronDown, MoreHorizontal } from '@lucide/svelte';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import LeaveTypeForm from './LeaveTypeForm.svelte';
	import PolicyEditor from './PolicyEditor.svelte';
	let { source }: { source: Promise<RouteLoadResult<LeaveType[]>> } = $props();
	const region = new LeaveRegion<LeaveType[]>(),
		request = new LatestRequest(),
		intent = new LeaveMutationIntent();
	let formOpen = $state(false),
		editing = $state<LeaveType | undefined>(),
		selected = $state<LeaveType | null>(null),
		policies = $state<LeavePolicyVersion[]>([]);
	let policySeed = $state<LeavePolicyVersion | undefined>();
	let expandedPolicies = $state<string[]>([]);
	let policiesLoading = $state(false),
		policiesError = $state(''),
		policyOpen = $state(false),
		deleting = $state<LeaveType | null>(null),
		pending = $state(''),
		error = $state(''),
		disposed = false;
	const allowed = $derived($can.has(PERMISSIONS.STAFF_LEAVE_MANAGE_SCHOOL));
	$effect.pre(() => region.consume(source));
	onDestroy(() => {
		disposed = true;
		request.abort();
		region.dispose();
	});
	function patch(type: LeaveType) {
		region.patch(
			[...(region.value ?? []).filter((value) => value.id !== type.id), type].sort((a, b) =>
				a.name.localeCompare(b.name, 'th')
			)
		);
		if (selected?.id === type.id) selected = type;
	}
	async function select(type: LeaveType) {
		const changed = selected?.id !== type.id;
		selected = type;
		if (changed) {
			policies = [];
			expandedPolicies = [];
		}
		policiesError = '';
		policiesLoading = true;
		const ticket = request.begin();
		try {
			const result = await listLeavePolicies(type.id, { signal: ticket.signal });
			if (request.isCurrent(ticket.revision) && !disposed) {
				policies = result;
				if (changed) expandedPolicies = result[0] ? [result[0].id] : [];
			}
		} catch (cause) {
			if (request.isCurrent(ticket.revision) && !disposed)
				policiesError = cause instanceof Error ? cause.message : 'โหลดนโยบายไม่สำเร็จ';
		} finally {
			if (request.isCurrent(ticket.revision) && !disposed) policiesLoading = false;
		}
	}
	async function archive(type: LeaveType) {
		if (pending || !allowed) return;
		pending = type.id;
		error = '';
		const basis = {
			expectedRevision: type.revision,
			name: type.name,
			kind: type.kind,
			archived: !type.archived
		};
		try {
			const result = await saveLeaveType(
				{
					requestId: intent.for({ operation: 'archive-type', typeId: type.id, ...basis }),
					...basis
				},
				type.id
			);
			if (!disposed) patch(result);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'เปลี่ยนสถานะไม่สำเร็จ';
		} finally {
			if (!disposed) pending = '';
		}
	}
	async function remove() {
		if (!deleting || pending || !allowed) return;
		pending = deleting.id;
		error = '';
		const target = deleting,
			basis = { expectedRevision: target.revision };
		try {
			await deleteLeaveType(target.id, {
				requestId: intent.for({ operation: 'delete-type', typeId: target.id, ...basis }),
				...basis
			});
			if (!disposed) {
				region.patch((region.value ?? []).filter((value) => value.id !== target.id));
				if (selected?.id === target.id) {
					selected = null;
					policies = [];
					request.abort();
				}
				deleting = null;
			}
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ลบประเภทไม่สำเร็จ';
		} finally {
			if (!disposed) pending = '';
		}
	}
</script>

<PageShell
	contentClass="space-y-4"
	title="ประเภทคำขอและเกณฑ์"
	description="กำหนดประเภท เงื่อนไข และผู้พิจารณาตามแนวปฏิบัติของโรงเรียน"
	backHref="/staff/leave"
>
	{#snippet actions()}{#if allowed}<Button
				onclick={() => {
					editing = undefined;
					formOpen = true;
				}}>เพิ่มประเภทคำขอ</Button
			>{/if}{/snippet}
	{#if error && !deleting}<PageState
			variant="error"
			title="ดำเนินการไม่สำเร็จ"
			description={error}
		/>{/if}
	<div class="grid items-start gap-4 lg:grid-cols-[minmax(15rem,0.7fr)_minmax(0,1.8fr)]">
		<section data-testid="leave-types" aria-busy={region.loading} class="min-w-0">
			{#if region.error}<PageState
					variant="error"
					title="โหลดประเภทคำขอไม่สำเร็จ"
					description={region.error}
					actionLabel="ลองอีกครั้ง"
					onaction={() =>
						region.retry((signal) => listLeaveTypes({ signal, query: { includeArchived: true } }))}
				/>{/if}
			{#if region.loading && !region.value}<div role="status" aria-label="กำลังโหลดประเภทคำขอ">
					<TableSkeleton columns={2} />
				</div>
			{:else if region.value?.length}<div class="overflow-hidden rounded-lg border bg-card">
					<h2 class="border-b px-3 py-2 text-sm font-medium">ประเภทคำขอ</h2>
					<ul class="divide-y">
						{#each region.value as type (type.id)}<li
								class="flex items-center gap-1 p-1.5 {selected?.id === type.id
									? 'bg-primary/5'
									: ''}"
							>
								<Button
									type="button"
									variant="ghost"
									class="h-auto min-w-0 flex-1 flex-col items-start gap-0 px-2 py-2 text-left whitespace-normal"
									aria-pressed={selected?.id === type.id}
									aria-label={`ดูเกณฑ์ ${type.name}`}
									onclick={() => select(type)}
									><span class="block break-words font-medium">{type.name}</span><span
										class="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
										>{leaveKindLabels[type.kind]}{#if type.archived}<Badge variant="outline"
												>เก็บถาวร</Badge
											>{:else}<span>เปิดใช้งาน</span>{/if}</span
									></Button
								>
								{#if allowed}<DropdownMenu.Root
										><DropdownMenu.Trigger
											>{#snippet child({ props })}<Button
													{...props}
													size="icon"
													variant="ghost"
													disabled={!!pending}
													aria-label={`จัดการประเภท ${type.name}`}
													><MoreHorizontal class="size-4" /></Button
												>{/snippet}</DropdownMenu.Trigger
										><DropdownMenu.Content align="end"
											><DropdownMenu.Group
												><DropdownMenu.GroupHeading>{type.name}</DropdownMenu.GroupHeading
												><DropdownMenu.Item
													onclick={() => {
														editing = type;
														formOpen = true;
													}}>แก้ไข {type.name}</DropdownMenu.Item
												><DropdownMenu.Item onclick={() => archive(type)}
													>{type.archived ? 'เปิดใช้งานอีกครั้ง' : 'เก็บถาวร'}
													{type.name}</DropdownMenu.Item
												><DropdownMenu.Separator /><DropdownMenu.Item
													class="text-destructive"
													onclick={() => {
														error = '';
														deleting = type;
													}}>ลบ {type.name}</DropdownMenu.Item
												></DropdownMenu.Group
											></DropdownMenu.Content
										></DropdownMenu.Root
									>{/if}
							</li>{/each}
					</ul>
				</div>
			{:else if !region.error}<PageState
					title="ยังไม่มีประเภทคำขอ"
					description="เพิ่มประเภทคำขอ แล้วกำหนดเกณฑ์และผู้พิจารณา"
				/>{/if}
		</section>
		{#if selected}<section
				data-testid="leave-policies"
				aria-busy={policiesLoading}
				class="min-w-0 space-y-3"
			>
				<div class="flex flex-wrap items-center justify-between gap-3">
					<h2 class="text-lg font-semibold">เกณฑ์: {selected.name}</h2>
					{#if allowed && !selected.archived}<Button
							onclick={() => {
								policySeed = undefined;
								policyOpen = true;
							}}>เพิ่มเกณฑ์ฉบับใหม่</Button
						>{/if}
				</div>
				{#if policiesError}<PageState
						variant="error"
						title="โหลดนโยบายไม่สำเร็จ"
						description={policiesError}
						actionLabel="ลองอีกครั้ง"
						onaction={() => {
							if (selected) void select(selected);
						}}
					/>{/if}{#if policiesLoading && !policies.length}<PageSkeleton
						variant="detail"
					/>{:else if policies.length}<div class="space-y-3">
						{#each policies as policy (policy.id)}<Collapsible.Root
								open={expandedPolicies.includes(policy.id)}
								onOpenChange={(open) =>
									(expandedPolicies = open
										? [...expandedPolicies, policy.id]
										: expandedPolicies.filter((id) => id !== policy.id))}
								class="rounded-lg border bg-card"
							>
								<Collapsible.Trigger
									>{#snippet child({ props })}<Button
											{...props}
											variant="ghost"
											class="group h-auto w-full flex-col items-start gap-0 rounded-lg p-3 text-left whitespace-normal"
											aria-label={`เกณฑ์ฉบับ ${policy.version} มีผล ${leaveDate(policy.effectiveFrom)}`}
											><span class="flex w-full items-center justify-between gap-2 font-medium"
												>ฉบับ {policy.version} · มีผล {leaveDate(policy.effectiveFrom)}<ChevronDown
													class="size-4 shrink-0 transition-transform group-data-[state=open]:rotate-180"
												/></span
											><span class="mt-1 block text-muted-foreground"
												>{policy.personnelTypes
													.map((value) => PERSONNEL_TYPE_LABELS[value])
													.join(' · ')}</span
											><span class="mt-1 block"
												>ต่อคำขอ {policy.maxPerRequest == null
													? 'ไม่กำหนด'
													: leaveAmount(policy.maxPerRequest, policy.unit)} · ต่อรอบปี {policy.annualLimit ==
												null
													? 'ไม่กำหนด'
													: leaveAmount(policy.annualLimit, policy.unit)} · {policy.steps.length} ขั้นตอน</span
											>{#if !policy.enabled}<Badge variant="outline" class="mt-2"
													>งดรับคำขอสำหรับกลุ่มนี้</Badge
												>{/if}</Button
										>{/snippet}</Collapsible.Trigger
								><Collapsible.Content
									><Card.Root class="gap-3 rounded-none border-0 border-t py-3 shadow-none"
										><Card.Content class="space-y-2 px-3 text-sm"
											><p class="break-words">อ้างอิง: {policy.sourceReference}</p>
											<p>
												นับ{policy.countingMode === 'working_days'
													? 'วันทำงานที่กำหนด'
													: 'วันปฏิทิน'} ·
												{policy.evidenceRequired
													? 'ต้องแนบหลักฐานทุกคำขอ'
													: policy.evidenceFromAmount != null
														? `ต้องแนบตั้งแต่ ${leaveAmount(policy.evidenceFromAmount, policy.unit)}`
														: 'หลักฐานตามความจำเป็น'}
											</p>
											<ol class="space-y-1">
												{#each policy.steps as step, index (index)}<li>
														{index + 1}. {step.title}{step.requiresSignature
															? ' · ใช้ลายมือชื่อ'
															: ''}
													</li>{/each}
											</ol>
											{#if allowed && !selected.archived}<Button
													type="button"
													variant="outline"
													onclick={() => {
														policySeed = policy;
														policyOpen = true;
													}}>สร้างฉบับใหม่จากฉบับนี้</Button
												>{/if}</Card.Content
										></Card.Root
									>
								</Collapsible.Content></Collapsible.Root
							>{/each}
					</div>{:else if !policiesError}<PageState
						title="ยังไม่มีนโยบายสำหรับประเภทนี้"
						description="กำหนดประเภทบุคลากร วันที่มีผล และผู้พิจารณาก่อนเปิดรับคำขอ"
					/>{/if}
			</section>{:else}<PageState
				title="เลือกประเภทคำขอ"
				description="เลือกประเภทเพื่อดูเกณฑ์และผู้พิจารณาของโรงเรียน"
			/>{/if}
	</div>
</PageShell>

<Dialog.Root bind:open={formOpen}
	><Dialog.Content
		><Dialog.Header
			><Dialog.Title>{editing ? 'แก้ไขประเภทคำขอ' : 'เพิ่มประเภทคำขอ'}</Dialog.Title
			><Dialog.Description>แยกกลุ่มการลาและการออกนอกโรงเรียน</Dialog.Description></Dialog.Header
		>{#if formOpen && allowed}<LeaveTypeForm
				type={editing}
				oncancel={() => (formOpen = false)}
				onsaved={(type) => {
					patch(type);
					formOpen = false;
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
<Dialog.Root bind:open={policyOpen}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-3xl"
		><Dialog.Header
			><Dialog.Title>เพิ่มเกณฑ์: {selected?.name}</Dialog.Title><Dialog.Description
				>บันทึกเป็นฉบับใหม่ โดยเก็บนโยบายและคำขอเดิมไว้</Dialog.Description
			></Dialog.Header
		>{#if policyOpen && selected && allowed}<PolicyEditor
				type={selected}
				initialPolicy={policySeed}
				oncancel={() => (policyOpen = false)}
				onsaved={(policy) => {
					policies = [policy, ...policies];
					expandedPolicies = [policy.id];
					if (selected) patch({ ...selected, revision: policy.typeRevision });
					policyOpen = false;
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
<AlertDialog.Root
	open={!!deleting}
	onOpenChange={(open) => {
		if (!open && !pending) deleting = null;
	}}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ลบประเภท {deleting?.name}?</AlertDialog.Title><AlertDialog.Description
				>ลบได้เฉพาะประเภทที่ยังไม่มีนโยบายหรือคำขอใช้งาน หากมีประวัติแล้วให้เก็บถาวร</AlertDialog.Description
			></AlertDialog.Header
		>{#if error}<PageState
				variant="error"
				title="ลบประเภทไม่ได้"
				description={error}
			/>{/if}<AlertDialog.Footer
			><AlertDialog.Cancel disabled={!!pending}>กลับ</AlertDialog.Cancel><LoadingButton
				variant="destructive"
				loading={!!pending}
				onclick={remove}>ยืนยันลบประเภท</LoadingButton
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
