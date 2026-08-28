<script lang="ts">
	import type {
		Homeroom,
		HomeroomPlacement,
		HomeroomPlacementTransfer
	} from '$lib/api/academic-core';
	import { Button } from '$lib/components/ui/button';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Textarea } from '$lib/components/ui/textarea';
	import { ArrowRightLeft } from 'lucide-svelte';

	let {
		open,
		placement,
		homerooms,
		onClose,
		onTransfer
	}: {
		open: boolean;
		placement: HomeroomPlacement | null;
		homerooms: Homeroom[];
		onClose: () => void;
		onTransfer: (draft: {
			targetHomeroomId: string;
			transferDate: string;
			enrollmentType: string;
			classNumber: number | null;
			reason: string;
		}) => Promise<HomeroomPlacementTransfer>;
	} = $props();

	let draft = $state(emptyDraft());
	let result = $state<HomeroomPlacementTransfer | null>(null);
	let busy = $state(false);
	let errorMessage = $state('');

	function emptyDraft() {
		return {
			targetHomeroomId: '',
			transferDate: '',
			enrollmentType: 'room_transfer',
			classNumber: null as number | null,
			reason: ''
		};
	}

	function roomName(id: string): string {
		return homerooms.find((room) => room.id === id)?.name ?? 'ไม่พบห้องประจำชั้นที่อ้างอิง';
	}

	function close() {
		draft = emptyDraft();
		result = null;
		errorMessage = '';
		onClose();
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!draft.targetHomeroomId || !draft.transferDate) {
			errorMessage = 'กรุณาเลือกห้องใหม่และวันที่ย้าย';
			return;
		}
		busy = true;
		errorMessage = '';
		try {
			result = await onTransfer(draft);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'ย้ายห้องไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<Dialog.Root {open} onOpenChange={(nextOpen) => !nextOpen && close()}>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>ย้ายห้องระหว่างปี</Dialog.Title>
			<Dialog.Description
				>รายการเดิมจะสิ้นสุดก่อนวันที่ย้ายหนึ่งวัน และสร้างรายการใหม่ต่อเนื่อง</Dialog.Description
			>
		</Dialog.Header>
		{#if placement}
			{#if result}
				<div class="space-y-4">
					<div class="rounded-xl border bg-muted/20 p-4">
						<p class="text-xs text-muted-foreground">รายการเดิมสิ้นสุด</p>
						<p class="mt-1 font-medium">
							{roomName(result.endedPlacement.homeroomId)} · {result.endedPlacement.endDate}
						</p>
					</div>
					<div class="flex justify-center"><ArrowRightLeft class="size-5 text-primary" /></div>
					<div class="rounded-xl border border-primary/30 bg-primary/5 p-4">
						<p class="text-xs text-muted-foreground">รายการใหม่</p>
						<p class="mt-1 font-medium">
							{roomName(result.newPlacement.homeroomId)} · เริ่ม {result.newPlacement.startDate}
						</p>
					</div>
					<Dialog.Footer><Button type="button" onclick={close}>เสร็จสิ้น</Button></Dialog.Footer>
				</div>
			{:else}
				<form class="space-y-4" onsubmit={submit}>
					<div class="space-y-1.5">
						<Label for="transfer-target">ห้องใหม่</Label><Select.Root
							type="single"
							bind:value={draft.targetHomeroomId}
							><Select.Trigger id="transfer-target" class="w-full"
								>{homerooms.find((room) => room.id === draft.targetHomeroomId)?.name ??
									'เลือกห้องใหม่'}</Select.Trigger
							><Select.Content
								>{#each homerooms.filter((room) => room.id !== placement.homeroomId) as room (room.id)}<Select.Item
										value={room.id}>{room.name}</Select.Item
									>{/each}</Select.Content
							></Select.Root
						>
					</div>
					<div class="grid grid-cols-2 gap-3">
						<div class="space-y-1.5">
							<Label for="transfer-date">วันที่ย้าย</Label><DatePicker
								id="transfer-date"
								bind:value={draft.transferDate}
								ariaLabel="เลือกวันที่ย้ายห้อง"
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for="transfer-number">เลขที่ใหม่</Label><Input
								id="transfer-number"
								type="number"
								min="1"
								bind:value={draft.classNumber}
							/>
						</div>
					</div>
					<div class="space-y-1.5">
						<Label for="transfer-reason">เหตุผลการย้าย</Label><Textarea
							id="transfer-reason"
							class="min-h-24"
							maxlength={500}
							bind:value={draft.reason}
							required
						/>
						<p class="text-right text-xs text-muted-foreground">{draft.reason.length}/500</p>
					</div>
					{#if errorMessage}<p
							role="alert"
							class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
						>
							{errorMessage}
						</p>{/if}
					<Dialog.Footer
						><Button type="button" variant="outline" onclick={close}>ยกเลิก</Button><Button
							type="submit"
							disabled={busy || !draft.reason.trim()}
							><ArrowRightLeft class="size-4" /> ยืนยันการย้าย</Button
						></Dialog.Footer
					>
				</form>
			{/if}
		{/if}
	</Dialog.Content>
</Dialog.Root>
