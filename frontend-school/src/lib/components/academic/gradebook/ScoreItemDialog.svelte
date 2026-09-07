<script lang="ts">
	import { untrack } from 'svelte';
	import type {
		GradebookItemInput,
		GradebookScoreItem,
		GroupPhaseWorkspace
	} from '$lib/api/academicGradebook';
	import { scoreItemBudget } from '$lib/academic/gradebook/item-budget';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Trash2 } from 'lucide-svelte';

	let {
		open,
		item,
		workspace,
		nextDisplayOrder,
		busy = false,
		onopenchange,
		onsave,
		onremove
	}: {
		open: boolean;
		item: GradebookScoreItem | null;
		workspace: GroupPhaseWorkspace;
		nextDisplayOrder: number;
		busy?: boolean;
		onopenchange: (open: boolean) => void;
		onsave: (input: GradebookItemInput) => void;
		onremove: (item: GradebookScoreItem) => void;
	} = $props();

	let name = $state(untrack(() => item?.name ?? ''));
	let maxScore = $state(untrack(() => item?.maxScore ?? ''));
	let displayOrder = $state(untrack(() => item?.displayOrder ?? nextDisplayOrder));
	let localError = $state('');
	let budget = $derived(scoreItemBudget(workspace.phaseMaxScore, workspace.items, item?.id));
	let minimumScore = $derived(
		Math.max(
			0,
			...workspace.scores
				.filter((score) => score.scoreItemId === item?.id)
				.map((score) => Number(score.value ?? 0))
		)
	);

	function submit(event: SubmitEvent) {
		event.preventDefault();
		const cleanName = name.trim();
		const cleanScore = maxScore.trim();
		if (!cleanName) {
			localError = 'กรุณาตั้งชื่อรายการคะแนน';
			return;
		}
		if (!/^(0|[1-9]\d*)(\.\d{1,2})?$/.test(cleanScore)) {
			localError = 'คะแนนเต็มต้องเป็น 0 ขึ้นไป และมีทศนิยมไม่เกิน 2 ตำแหน่ง';
			return;
		}
		if (!item && !budget.canAdd) {
			localError = 'จัดสรรคะแนนครบแล้ว กรุณาลดคะแนนเต็มของรายการเดิมก่อนเพิ่มรายการใหม่';
			return;
		}
		if (Number(cleanScore) > budget.maximumForItem) {
			localError = `คะแนนเต็มของรายการนี้ต้องไม่เกิน ${budget.maximumForItem} คะแนน`;
			return;
		}
		if (Number(cleanScore) < minimumScore) {
			localError = `มีคะแนนนักเรียนสูงสุด ${minimumScore} คะแนน กรุณาปรับหรือเคลียร์คะแนนนั้นก่อนลดคะแนนเต็ม`;
			return;
		}
		localError = '';
		onsave({
			name: cleanName,
			maxScore: cleanScore,
			displayOrder,
			rowVersion: item?.rowVersion ?? null
		});
	}
</script>

<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content class="sm:max-w-md">
		<form class="space-y-5" onsubmit={submit}>
			<Dialog.Header>
				<Dialog.Title>{item ? 'แก้รายการคะแนน' : 'เพิ่มรายการคะแนน'}</Dialog.Title>
				<Dialog.Description>
					คะแนนเต็มของรายการที่ใช้งานอยู่รวมกันต้องเท่ากับคะแนนเต็มของช่วง
				</Dialog.Description>
			</Dialog.Header>

			<div class="space-y-4">
				<p id="score-item-budget" class="rounded-lg bg-muted/40 p-3 text-sm">
					ช่วงนี้เต็ม {workspace.phaseMaxScore} คะแนน · รายการนี้กำหนดได้ไม่เกิน {budget.maximumForItem}
					คะแนน
					{#if budget.remaining < 0}<span class="mt-1 block text-destructive"
							>ยอดรวมเกิน {-budget.remaining} คะแนน ยังแก้ชื่อหรือลดคะแนนเต็มทีละรายการได้</span
						>{/if}
					{#if minimumScore > 0}<span class="mt-1 block text-muted-foreground"
							>มีคะแนนนักเรียนสูงสุด {minimumScore} คะแนน ระบบจะไม่ปรับคะแนนนักเรียนอัตโนมัติ</span
						>{/if}
				</p>
				<div class="space-y-1.5">
					<Label for="score-item-name">ชื่อรายการ</Label>
					<Input id="score-item-name" bind:value={name} placeholder="เช่น ชีท 1 หรือ ปรนัย" />
				</div>
				<div class="grid grid-cols-2 gap-3">
					<div class="space-y-1.5">
						<Label for="score-item-max">คะแนนเต็ม</Label>
						<Input
							id="score-item-max"
							inputmode="decimal"
							bind:value={maxScore}
							aria-describedby="score-item-budget"
						/>
					</div>
					<div class="space-y-1.5">
						<Label for="score-item-order">ลำดับ</Label>
						<Input id="score-item-order" type="number" min="1" bind:value={displayOrder} />
					</div>
				</div>
				{#if localError}<p class="text-sm text-destructive">{localError}</p>{/if}
			</div>

			<Dialog.Footer class="gap-2 sm:justify-between">
				{#if item}
					<Button
						type="button"
						variant="destructive"
						disabled={busy}
						onclick={() => onremove(item)}
					>
						<Trash2 class="size-4" /> นำรายการออก
					</Button>
				{:else}<span></span>{/if}
				<div class="flex justify-end gap-2">
					<Button
						type="button"
						variant="outline"
						disabled={busy}
						onclick={() => onopenchange(false)}>ยกเลิก</Button
					>
					<LoadingButton type="submit" loading={busy}
						>{item ? 'บันทึกการแก้ไข' : 'เพิ่มรายการ'}</LoadingButton
					>
				</div>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
