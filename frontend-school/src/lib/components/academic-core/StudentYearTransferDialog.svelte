<script lang="ts">
	import type {
		Homeroom,
		HomeroomPlacement,
		HomeroomPlacementTransfer
	} from '$lib/api/academic-core';
	import { Button } from '$lib/components/ui/button';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { ArrowRightLeft, X } from 'lucide-svelte';

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

	let draft = $state({
		targetHomeroomId: '',
		transferDate: '',
		enrollmentType: 'room_transfer',
		classNumber: null as number | null,
		reason: ''
	});
	let result = $state<HomeroomPlacementTransfer | null>(null);
	let busy = $state(false);
	let errorMessage = $state('');

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

{#if open && placement}
	<div class="fixed inset-0 z-50 grid place-items-center bg-black/45 p-4" role="presentation">
		<div
			role="dialog"
			aria-modal="true"
			aria-labelledby="transfer-dialog-title"
			class="w-full max-w-lg rounded-xl border bg-background shadow-2xl"
		>
			<header class="flex items-start justify-between border-b px-5 py-4">
				<div>
					<h2 id="transfer-dialog-title" class="font-semibold">ย้ายห้องระหว่างปี</h2>
					<p class="text-xs text-muted-foreground">
						รายการเดิมจะสิ้นสุดก่อนวันที่ย้ายหนึ่งวัน และสร้างรายการใหม่ต่อเนื่อง
					</p>
				</div>
				<Button size="icon" variant="ghost" onclick={onClose} aria-label="ปิด"
					><X class="size-4" /></Button
				>
			</header>
			{#if result}
				<div class="space-y-4 p-5">
					<div class="rounded-lg border bg-muted/20 p-4">
						<p class="text-xs text-muted-foreground">รายการเดิมสิ้นสุด</p>
						<p class="mt-1 font-medium">
							{result.endedPlacement.homeroomId} · {result.endedPlacement.endDate}
						</p>
					</div>
					<div class="flex justify-center"><ArrowRightLeft class="size-5 text-primary" /></div>
					<div class="rounded-lg border border-primary/30 bg-primary/5 p-4">
						<p class="text-xs text-muted-foreground">รายการใหม่</p>
						<p class="mt-1 font-medium">
							{result.newPlacement.homeroomId} · เริ่ม {result.newPlacement.startDate}
						</p>
					</div>
					<Button class="w-full" onclick={onClose}>เสร็จสิ้น</Button>
				</div>
			{:else}
				<form class="space-y-4 p-5" onsubmit={submit}>
					<div class="space-y-1.5">
						<Label for="transfer-target">ห้องใหม่</Label>
						<Select.Root type="single" bind:value={draft.targetHomeroomId}>
							<Select.Trigger id="transfer-target" class="w-full">
								{homerooms.find((room) => room.id === draft.targetHomeroomId)?.name ??
									'เลือกห้องใหม่'}
							</Select.Trigger>
							<Select.Content>
								{#each homerooms.filter((room) => room.id !== placement.homeroomId) as room (room.id)}
									<Select.Item value={room.id}>{room.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="grid grid-cols-2 gap-3">
						<div class="space-y-1.5">
							<Label for="transfer-date">วันที่ย้าย</Label>
							<DatePicker
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
						<Label for="transfer-reason">เหตุผลการย้าย</Label><textarea
							id="transfer-reason"
							class="min-h-24 w-full rounded-md border bg-background px-3 py-2 text-sm"
							maxlength="500"
							bind:value={draft.reason}
							required
						></textarea>
						<p class="text-right text-xs text-muted-foreground">{draft.reason.length}/500</p>
					</div>
					{#if errorMessage}<p
							role="alert"
							class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
						>
							{errorMessage}
						</p>{/if}
					<div class="flex justify-end gap-2">
						<Button type="button" variant="outline" onclick={onClose}>ยกเลิก</Button><Button
							type="submit"
							disabled={busy || !draft.reason.trim()}
							><ArrowRightLeft class="size-4" /> ยืนยันการย้าย</Button
						>
					</div>
				</form>
			{/if}
		</div>
	</div>
{/if}
