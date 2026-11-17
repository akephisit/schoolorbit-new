<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		getBudgetProject,
		deleteBudgetProject,
		type BudgetProjectWorkspace
	} from '#lib/api/budget.js';
	import { BudgetMutationIntent } from '#lib/budget/mutation-intent.js';
	import { message } from '#lib/budget/drafts.js';
	import { budgetDeletePermissions } from '#lib/budget/access.js';
	import { can } from '#lib/stores/permissions.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { PageState, LoadingButton } from '#lib/components/app-state/index.js';
	let {
		id,
		workspace,
		ondeleted
	}: { id: string; workspace?: BudgetProjectWorkspace; ondeleted: () => void } = $props();
	let open = $state(false),
		value = $state<BudgetProjectWorkspace | null>(null),
		loading = $state(false),
		pending = $state(false),
		error = $state(''),
		confirmation = $state('');
	let disposed = false;
	const controller = new AbortController(),
		intent = new BudgetMutationIntent();
	onDestroy(() => {
		disposed = true;
		controller.abort();
	});
	async function inspect() {
		open = true;
		error = '';
		confirmation = '';
		loading = true;
		value = null;
		try {
			const next = workspace ?? (await getBudgetProject(id, { signal: controller.signal }));
			if (!disposed) value = next;
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) loading = false;
		}
	}
	async function remove() {
		if (!value?.capabilities.canDelete || pending || confirmation !== value.project.name) return;
		pending = true;
		error = '';
		try {
			const basis = { expectedRevision: value.project.revision };
			await deleteBudgetProject(id, { ...basis, requestId: intent.for({ id, ...basis }) });
			if (!disposed) {
				open = false;
				ondeleted();
			}
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

{#if $can.hasAny(...budgetDeletePermissions) && (!workspace || workspace.capabilities.canDelete)}<Button
		variant="destructive"
		size="sm"
		disabled={pending || loading}
		onclick={inspect}>ลบโครงการ</Button
	>{/if}
<Dialog.Root bind:open
	><Dialog.Content
		><Dialog.Header
			><Dialog.Title>ลบโครงการถาวร</Dialog.Title><Dialog.Description
				>ลบโครงการที่ยังไม่เคยอนุมัติ พร้อมกิจกรรม ค่าใช้จ่าย ประวัติการเสนอแผน การพิจารณา
				และงานที่เกี่ยวข้องทั้งหมด ย้อนกลับไม่ได้</Dialog.Description
			></Dialog.Header
		>{#if loading}<p role="status">
				กำลังตรวจสอบขอบเขตการลบ…
			</p>{:else if value?.capabilities.canDelete}<p class="font-medium">{value.project.name}</p>
			<p class="text-sm">
				ขอบเขตที่จะลบ: {value.deletionImpact.projects} โครงการ · {value.deletionImpact.activities} กิจกรรม
				· {value.deletionImpact.planLines} รายการค่าใช้จ่าย · {value.deletionImpact.attempts} ครั้งที่เสนอแผน
				· {value.deletionImpact.workflowInstances} กระบวนการพิจารณา · {value.deletionImpact
					.historyEntries} รายการประวัติ รวมงานพิจารณาที่เกี่ยวข้องทั้งหมด
			</p>
			<Label for={`budget-delete-confirm-${id}`}>พิมพ์ชื่อโครงการเพื่อยืนยัน</Label><Input
				id={`budget-delete-confirm-${id}`}
				bind:value={confirmation}
				disabled={pending}
			/>{:else if value}<PageState
				title="โครงการนี้ลบไม่ได้"
				description="โครงการที่เคยอนุมัติ มีข้อมูลการเงิน อยู่ในปีที่ปิดแล้ว หรือเกี่ยวข้องกับการแก้ไขแผนที่อนุมัติแล้ว จะเก็บรักษาไว้"
			/>{/if}{#if error}<PageState
				variant="error"
				title="ลบโครงการไม่สำเร็จ"
				description={error}
				actionLabel={value ? undefined : 'ลองอีกครั้ง'}
				onaction={inspect}
			/>{/if}<Dialog.Footer
			><Button variant="outline" disabled={pending} onclick={() => (open = false)}>ยกเลิก</Button
			>{#if value?.capabilities.canDelete}<LoadingButton
					variant="destructive"
					loading={pending}
					disabled={confirmation !== value.project.name}
					onclick={remove}>ยืนยันลบโครงการถาวร</LoadingButton
				>{/if}</Dialog.Footer
		></Dialog.Content
	></Dialog.Root
>
