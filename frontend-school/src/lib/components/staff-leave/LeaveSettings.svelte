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
	import * as Table from '#lib/components/ui/table/index.js';
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
		if (changed) policies = [];
		policiesError = '';
		policiesLoading = true;
		const ticket = request.begin();
		try {
			const result = await listLeavePolicies(type.id, { signal: ticket.signal });
			if (request.isCurrent(ticket.revision) && !disposed) policies = result;
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
			code: type.code,
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
	title="ประเภทคำขอและนโยบายการลา"
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
	<section data-testid="leave-types" aria-busy={region.loading}>
		{#if region.error}<PageState
				variant="error"
				title="โหลดประเภทคำขอไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() =>
					region.retry((signal) => listLeaveTypes({ signal, query: { includeArchived: true } }))}
			/>{/if}
		{#if region.loading && !region.value}<div role="status" aria-label="กำลังโหลดประเภทคำขอ">
				<TableSkeleton columns={4} />
			</div>{:else if region.value?.length}<Table.Root
				><Table.Header
					><Table.Row
						><Table.Head>ประเภท</Table.Head><Table.Head>กลุ่ม</Table.Head><Table.Head
							>สถานะ</Table.Head
						><Table.Head>จัดการ</Table.Head></Table.Row
					></Table.Header
				><Table.Body
					>{#each region.value as type (type.id)}<Table.Row
							><Table.Cell
								><p class="font-medium">{type.name}</p>
								<p class="text-sm text-muted-foreground">{type.code}</p></Table.Cell
							><Table.Cell>{leaveKindLabels[type.kind]}</Table.Cell><Table.Cell
								><Badge variant={type.archived ? 'outline' : 'secondary'}
									>{type.archived ? 'เก็บถาวร' : 'เปิดใช้งาน'}</Badge
								></Table.Cell
							><Table.Cell
								><div class="flex flex-wrap gap-2">
									<Button variant="outline" size="sm" onclick={() => select(type)}
										>ดูนโยบาย {type.name}</Button
									>{#if allowed}<Button
											variant="ghost"
											size="sm"
											onclick={() => {
												editing = type;
												formOpen = true;
											}}>แก้ไข {type.name}</Button
										><LoadingButton
											variant="ghost"
											size="sm"
											loading={pending === type.id}
											disabled={!!pending}
											onclick={() => archive(type)}
											>{type.archived ? 'เปิดใช้งานอีกครั้ง' : 'เก็บถาวร'}</LoadingButton
										><Button
											variant="ghost"
											size="sm"
											disabled={!!pending}
											onclick={() => {
												error = '';
												deleting = type;
											}}>ลบ {type.name}</Button
										>{/if}
								</div></Table.Cell
							></Table.Row
						>{/each}</Table.Body
				></Table.Root
			>{:else if !region.error}<PageState
				title="ยังไม่มีประเภทคำขอ"
				description="เพิ่มประเภทคำขอ จากนั้นตั้งค่านโยบายและผู้พิจารณา"
			/>{/if}
	</section>
	{#if selected}<section data-testid="leave-policies" aria-busy={policiesLoading} class="space-y-4">
			<div class="flex flex-wrap items-center justify-between gap-3">
				<h2 class="text-lg font-semibold">นโยบาย: {selected.name}</h2>
				{#if allowed && !selected.archived}<Button
						onclick={() => {
							policySeed = undefined;
							policyOpen = true;
						}}>เพิ่มนโยบายฉบับใหม่</Button
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
				/>{:else if policies.length}<div class="grid gap-4 lg:grid-cols-2">
					{#each policies as policy (policy.id)}<Card.Root
							><Card.Header
								><Card.Title
									>ฉบับ {policy.version} · มีผล {leaveDate(policy.effectiveFrom)}</Card.Title
								>{#if !policy.enabled}<Badge variant="outline">งดรับคำขอสำหรับกลุ่มนี้</Badge
									>{/if}<Card.Description
									>{policy.personnelTypes
										.map((type) => PERSONNEL_TYPE_LABELS[type])
										.join(' · ')}</Card.Description
								></Card.Header
							><Card.Content class="space-y-2 text-sm"
								><p class="break-words">อ้างอิง: {policy.sourceReference}</p>
								<p>
									นับ{policy.countingMode === 'working_days' ? 'วันทำงานที่กำหนด' : 'วันปฏิทิน'} · {policy.evidenceRequired
										? 'ต้องแนบหลักฐาน'
										: 'หลักฐานตามความจำเป็น'}
								</p>
								<p>
									ต่อคำขอ: {policy.maxPerRequest == null
										? 'ไม่กำหนด'
										: leaveAmount(policy.maxPerRequest, policy.unit)} · ต่อรอบปี: {policy.annualLimit ==
									null
										? 'ไม่กำหนด'
										: leaveAmount(policy.annualLimit, policy.unit)}
								</p>
								<ol class="space-y-1">
									{#each policy.steps as step, index (index)}<li>
											{index + 1}. {step.title}{step.requiresSignature ? ' · ใช้ลายมือชื่อ' : ''}
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
						>{/each}
				</div>{:else if !policiesError}<PageState
					title="ยังไม่มีนโยบายสำหรับประเภทนี้"
					description="กำหนดประเภทบุคลากร วันที่มีผล และผู้พิจารณาก่อนเปิดรับคำขอ"
				/>{/if}
		</section>{/if}
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
			><Dialog.Title>เพิ่มนโยบาย: {selected?.name}</Dialog.Title><Dialog.Description
				>บันทึกเป็นฉบับใหม่ โดยเก็บนโยบายและคำขอเดิมไว้</Dialog.Description
			></Dialog.Header
		>{#if policyOpen && selected && allowed}<PolicyEditor
				type={selected}
				initialPolicy={policySeed}
				oncancel={() => (policyOpen = false)}
				onsaved={(policy) => {
					policies = [policy, ...policies];
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
