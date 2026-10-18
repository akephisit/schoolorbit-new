<script lang="ts">
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import CareStaffPicker from './CareStaffPicker.svelte';
	import { PageSkeleton } from '#lib/components/app-state/index.js';
	import { untrack, onDestroy } from 'svelte';
	import {
		careStaff,
		updateCareAssignmentReviewers,
		cancelCareAssignment,
		type CareStaffOption,
		type CareAssignment
	} from '#lib/api/student-care.js';
	import {
		careIdentity,
		mayReadCareAssignment,
		mayCancelCareAssignment
	} from '#lib/features/student-care/access.js';
	import { CareMutationIdentity } from '#lib/features/student-care/mutation.js';
	import { respondentLabels, careLink } from '#lib/features/student-care/presentation.js';
	import CareStatus from './CareStatus.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { authStore } from '#lib/stores/auth.js';
	let {
		items,
		term,
		allowManagement = false
	}: { items: CareAssignment[]; term?: string | null; allowManagement?: boolean } = $props();
	let rows = $state(untrack(() => items)),
		selected = $state<CareAssignment | null>(null),
		reason = $state(''),
		pending = $state(false),
		error = $state(''),
		open = $state(false);
	let staff = $state<CareStaffOption[] | null>(null),
		reviewerIds = $state<string[]>([]),
		reviewerOpen = $state(false),
		staffPending = $state(false),
		staffError = $state('');
	const staffRequest = new LatestRequest();
	let alive = true;
	onDestroy(() => {
		alive = false;
		staffRequest.abort();
	});
	const mayReassign = $derived(
		$can.hasAny(
			PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL,
			PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL
		)
	);
	async function loadStaff() {
		const token = staffRequest.begin();
		staffPending = true;
		staffError = '';
		const identity = $careIdentity;
		try {
			const options = await careStaff({ signal: token.signal });
			if (staffRequest.isCurrent(token.revision) && identity === $careIdentity) staff = options;
		} catch (cause) {
			if (staffRequest.isCurrent(token.revision) && identity === $careIdentity)
				staffError = cause instanceof Error ? cause.message : 'โหลดบุคลากรไม่ได้';
		} finally {
			if (staffRequest.isCurrent(token.revision) && identity === $careIdentity)
				staffPending = false;
		}
	}
	function handoff(item: CareAssignment) {
		selected = item;
		reviewerIds = [...item.reviewerUserIds];
		reason = '';
		error = '';
		reviewerOpen = true;
		void loadStaff();
	}
	async function reassign() {
		if (!selected || !mayReassign || pending || !reason.trim() || !reviewerIds.length) return;
		pending = true;
		error = '';
		const identity = $careIdentity;
		try {
			const row = await updateCareAssignmentReviewers(
				selected.id,
				mutation.prepare(
					{
						requestId: crypto.randomUUID(),
						rowVersion: selected.rowVersion,
						reviewerUserIds: reviewerIds,
						reason: reason.trim()
					},
					'assignment_reviewers'
				)
			);
			if (alive && identity === $careIdentity) {
				rows = rows.map((item) => (item.id === row.id ? row : item));
				reviewerOpen = false;
				selected = null;
			}
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'เปลี่ยนผู้รับผิดชอบไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) pending = false;
		}
	}
	const mutation = new CareMutationIdentity();
	function choose(item: CareAssignment) {
		selected = item;
		reason = '';
		error = '';
		open = true;
	}
	async function cancel() {
		if (!selected || !mayCancelCareAssignment(selected) || pending || !reason.trim()) return;
		pending = true;
		error = '';
		const identity = $careIdentity;
		try {
			const row = await cancelCareAssignment(
				selected.id,
				mutation.prepare(
					{
						requestId: crypto.randomUUID(),
						rowVersion: selected.rowVersion,
						reason: reason.trim()
					},
					'assignment_cancel'
				)
			);
			if (alive && identity === $careIdentity) {
				rows = rows.map((item) => (item.id === row.id ? row : item));
				open = false;
				selected = null;
			}
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'ยกเลิกไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) pending = false;
		}
	}
</script>

{#if !rows.length}<PageState
		title="ยังไม่มีแบบประเมินที่มอบหมาย"
		description="แบบประเมินที่อยู่ในสิทธิ์ของคุณจะแสดงที่นี่"
	/>{:else}<ul class="divide-y rounded-xl border bg-card">
		{#each rows as item (item.id)}<li class="flex flex-wrap items-center justify-between gap-3 p-4">
				<div class="min-w-0 space-y-1">
					<p class="break-words font-medium">{item.instrumentName}</p>
					<p class="text-sm text-muted-foreground">
						{item.studentName} · {respondentLabels[item.respondent]}
					</p>
					<CareStatus status={{ kind: 'assignment', value: item.status }} />
				</div>
				<div class="flex flex-wrap gap-2">
					{#if $authStore.user?.user_type === 'staff'}<Button
							href={careLink(`/students/${item.studentId}`, term)}
							variant="ghost"
							size="sm">ประวัติการดูแล</Button
						>{/if}{#if mayReadCareAssignment(item)}<Button
							href={`/student-care/assignments/${item.id}`}
							variant="outline"
							>{item.status === 'submitted' ? 'ดูรายละเอียด' : 'เปิดแบบประเมิน'}</Button
						>{/if}{#if allowManagement && mayReassign && item.status !== 'cancelled'}<Button
							variant="outline"
							onclick={() => handoff(item)}>เปลี่ยนผู้รับผิดชอบติดตาม</Button
						>{/if}{#if allowManagement && mayCancelCareAssignment(item)}<Button
							variant="ghost"
							onclick={() => choose(item)}>ยกเลิกการมอบหมาย</Button
						>{/if}
				</div>
			</li>{/each}
	</ul>{/if}
<AlertDialog.Root bind:open
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ยกเลิกการมอบหมายแบบประเมิน</AlertDialog.Title><AlertDialog.Description
				>{selected?.studentName} · {selected?.instrumentName} หลังยกเลิกจะตอบแบบประเมินนี้ต่อไม่ได้</AlertDialog.Description
			></AlertDialog.Header
		>
		<div class="space-y-2">
			<Label for="assignment-cancel-reason">เหตุผลการยกเลิก *</Label><Textarea
				id="assignment-cancel-reason"
				bind:value={reason}
				maxlength={1000}
			/>
		</div>
		{#if error}<PageState
				variant="error"
				title="ยกเลิกไม่ได้"
				description={error}
			/>{/if}<AlertDialog.Footer
			><AlertDialog.Cancel disabled={pending}>กลับ</AlertDialog.Cancel><LoadingButton
				loading={pending}
				disabled={!reason.trim()}
				onclick={cancel}>ยืนยันยกเลิกการมอบหมาย</LoadingButton
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>

<Dialog.Root bind:open={reviewerOpen}
	><Dialog.Content
		><Dialog.Header
			><Dialog.Title>เปลี่ยนผู้รับผิดชอบติดตามผล</Dialog.Title><Dialog.Description
				>{selected?.studentName} · {selected?.instrumentName} ระบุผู้รับผิดชอบที่มีสิทธิ์ดูแลข้อมูลละเอียด</Dialog.Description
			></Dialog.Header
		>{#if staffPending}<PageSkeleton variant="detail" />{:else if staffError}<PageState
				variant="error"
				title="โหลดบุคลากรไม่ได้"
				description={staffError}
				actionLabel="ลองโหลดบุคลากรใหม่"
				onaction={loadStaff}
			/>{:else if staff}<CareStaffPicker
				{staff}
				value={reviewerIds}
				onchange={(value) => (reviewerIds = value)}
				label="ผู้รับผิดชอบติดตามผล"
			/>{/if}
		<div class="space-y-2">
			<Label for="assignment-handoff-reason">เหตุผลการเปลี่ยนผู้รับผิดชอบ *</Label><Textarea
				id="assignment-handoff-reason"
				bind:value={reason}
				maxlength={1000}
			/>
		</div>
		{#if error}<PageState
				variant="error"
				title="เปลี่ยนผู้รับผิดชอบไม่ได้"
				description={error}
			/>{/if}<Dialog.Footer
			><Button variant="outline" disabled={pending} onclick={() => (reviewerOpen = false)}
				>ยกเลิก</Button
			><LoadingButton
				loading={pending}
				disabled={staffPending || !!staffError || !reviewerIds.length || !reason.trim()}
				onclick={reassign}>บันทึกผู้รับผิดชอบติดตาม</LoadingButton
			></Dialog.Footer
		></Dialog.Content
	></Dialog.Root
>
