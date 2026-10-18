<script lang="ts">
	import { untrack, onDestroy } from 'svelte';
	import { updateCareRound, type CareRound, type CareRoundStatus } from '#lib/api/student-care.js';
	import { CareMutationIdentity } from '#lib/features/student-care/mutation.js';
	import { careIdentity, careManagePermissions } from '#lib/features/student-care/access.js';
	import { careDate, roundLabels, careLink } from '#lib/features/student-care/presentation.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import CareSection from './CareSection.svelte';
	import CareSelect from './CareSelect.svelte';
	import CareStatus from './CareStatus.svelte';
	let { initial }: { initial: CareRound } = $props();
	let round = $state(untrack(() => initial)),
		status = $state<CareRoundStatus>(
			untrack(() => (initial.status === 'open' ? 'closed' : 'cancelled'))
		),
		pending = $state(false),
		error = $state(''),
		confirm = $state(false);
	let alive = true;
	onDestroy(() => (alive = false));
	const mutation = new CareMutationIdentity();
	const transitions = $derived(
		Object.entries(roundLabels)
			.filter(([value]) =>
				round.status === 'open'
					? value === 'closed' || value === 'cancelled'
					: round.status === 'closed'
						? value === 'cancelled'
						: false
			)
			.map(([value, label]) => ({ value, label }))
	);
	async function save() {
		if (
			pending ||
			!$can.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL) ||
			!transitions.some((option) => option.value === status)
		)
			return;
		pending = true;
		error = '';
		const identity = $careIdentity;
		try {
			const next = await updateCareRound(
				round.id,
				mutation.prepare(
					{ requestId: crypto.randomUUID(), rowVersion: round.rowVersion, status },
					'update_round'
				)
			);
			if (alive && identity === $careIdentity) {
				round = next;
				status = 'cancelled';
				confirm = false;
			}
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'บันทึกไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) pending = false;
		}
	}
</script>

<CareSection title={round.name}
	><CareStatus status={{ kind: 'round', value: round.status }} />
	<p class="whitespace-pre-wrap text-sm">{round.purpose}</p>
	<dl class="grid gap-3 text-sm sm:grid-cols-2">
		<div>
			<dt class="text-muted-foreground">วันที่อ้างอิงรายชื่อ</dt>
			<dd>{careDate(round.rosterDate)}</dd>
		</div>
		<div>
			<dt class="text-muted-foreground">กำหนดส่ง</dt>
			<dd>{careDate(round.dueDate)}</dd>
		</div>
	</dl>
	<p class="text-sm text-muted-foreground">
		เลือกไว้ {round.instrumentVersionIds.length} รุ่นแบบประเมิน
	</p>
	{#if $can.hasAny(...careManagePermissions) && round.status === 'open'}<Button
			href={careLink(`/rounds/${round.id}/assign`, round.academicTermId)}>มอบหมายผู้ตอบ</Button
		>{/if}{#if $can.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL) && transitions.length}<div
			class="flex flex-wrap items-end gap-3"
		>
			<CareSelect
				id="round-status"
				label="เปลี่ยนสถานะรอบเป็น"
				value={status}
				options={transitions}
				onValueChange={(value) => {
					if (value === 'closed' || value === 'cancelled') status = value;
				}}
			/><Button
				variant="outline"
				onclick={() => {
					error = '';
					confirm = true;
				}}>เปลี่ยนสถานะรอบ</Button
			>
		</div>{/if}{#if error && !confirm}<PageState
			variant="error"
			title="บันทึกรอบไม่ได้"
			description={error}
		/>{/if}</CareSection
>
<AlertDialog.Root bind:open={confirm}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ยืนยัน{roundLabels[status]}</AlertDialog.Title><AlertDialog.Description
				>เมื่อปิดหรือยกเลิกรอบแล้ว จะไม่สามารถกลับมาเปิดรับคำตอบในรอบนี้ได้
				ประวัติที่บันทึกไว้ยังคงอยู่</AlertDialog.Description
			></AlertDialog.Header
		>{#if error}<PageState
				variant="error"
				title="บันทึกรอบไม่ได้"
				description={error}
			/>{/if}<AlertDialog.Footer
			><AlertDialog.Cancel disabled={pending}>กลับไปตรวจ</AlertDialog.Cancel><LoadingButton
				loading={pending}
				onclick={save}>ยืนยันเปลี่ยนสถานะรอบ</LoadingButton
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
