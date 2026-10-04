<script lang="ts">
	import {
		createAcademicTermChangeSet,
		type AcademicTermChangeSet
	} from '#lib/api/learning-delivery.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { CalendarClock, TriangleAlert } from '@lucide/svelte';

	let {
		academicTermId,
		onCreated,
		showTrigger = true
	}: {
		academicTermId: string;
		onCreated: (changeSet: AcademicTermChangeSet) => void | Promise<void>;
		showTrigger?: boolean;
	} = $props();

	let open = $state(false);
	let reason = $state('');
	let saving = $state(false);
	let errorMessage = $state('');
	let creationKey = '';
	let submittedReason = '';

	export function openDialog() {
		errorMessage = '';
		open = true;
	}

	async function createDraft(event: SubmitEvent) {
		event.preventDefault();
		if (!reason.trim()) return;
		saving = true;
		errorMessage = '';
		if (submittedReason !== reason.trim() || !creationKey) {
			submittedReason = reason.trim();
			creationKey = crypto.randomUUID();
		}
		try {
			const changeSet = await createAcademicTermChangeSet({
				academicTermId,
				reason: reason.trim(),
				idempotencyKey: creationKey
			});
			await onCreated(changeSet);
			reason = '';
			open = false;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างแบบร่างรุ่นเปิดสอนไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

{#if showTrigger}<Button variant="outline" onclick={openDialog}
		><CalendarClock class="size-4" />สร้างรุ่นเปิดสอน</Button
	>{/if}

<Dialog.Root bind:open>
	<Dialog.Content
		showCloseButton={!saving}
		escapeKeydownBehavior={saving ? 'ignore' : 'close'}
		interactOutsideBehavior={saving ? 'ignore' : 'close'}
		class="sm:max-w-xl"
	>
		<Dialog.Header
			><Dialog.Title>สร้างรุ่นเปิดสอน</Dialog.Title><Dialog.Description
				>กำหนดรายวิชา กลุ่ม ครู และจำนวนคาบของภาคเรียน ก่อนเผยแพร่ข้อมูลให้ใช้จัดตาราง</Dialog.Description
			></Dialog.Header
		>
		<div class="flex gap-3 rounded-xl border bg-muted/30 p-3 text-sm">
			<TriangleAlert class="size-4 shrink-0" />
			<p>
				ตารางที่เผยแพร่แล้วจะใช้ข้อมูลรุ่นเดิมต่อไป
				การเผยแพร่รุ่นเปิดสอนใหม่ไม่เปลี่ยนตารางโดยอัตโนมัติ
			</p>
		</div>

		<form class="space-y-4" onsubmit={createDraft}>
			<div class="space-y-2">
				<Label for="academic-change-reason">ชื่อหรือเหตุผลของรุ่น</Label><Textarea
					id="academic-change-reason"
					bind:value={reason}
					rows={3}
					disabled={saving}
					placeholder="เช่น เปลี่ยนครูหรือปรับจำนวนคาบตามมติฝ่ายวิชาการ"
					required
				/>
			</div>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
			<Dialog.Footer>
				<Button type="button" variant="outline" disabled={saving} onclick={() => (open = false)}
					>กลับ</Button
				>
				<LoadingButton
					type="submit"
					loading={saving}
					loadingLabel="กำลังสร้างแบบร่าง"
					disabled={!reason.trim()}
				>
					สร้างแบบร่าง
				</LoadingButton>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
