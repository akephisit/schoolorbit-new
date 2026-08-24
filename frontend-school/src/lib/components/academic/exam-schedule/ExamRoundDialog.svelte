<script lang="ts">
	import type { CreateExamRoundInput, ExamRoundKind } from '$lib/api/examSchedule';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Textarea } from '$lib/components/ui/textarea';

	let {
		open = $bindable(false),
		academicTermId,
		termLabel,
		saving = false,
		onCreate
	}: {
		open?: boolean;
		academicTermId: string;
		termLabel: string;
		saving?: boolean;
		onCreate?: (input: CreateExamRoundInput) => Promise<boolean> | boolean;
	} = $props();

	let name = $state('');
	let examKind = $state<ExamRoundKind>('midterm');
	let description = $state('');

	function resetForm() {
		name = '';
		examKind = 'midterm';
		description = '';
	}

	async function submitForm() {
		if (!name.trim() || !academicTermId) return;

		const created = await onCreate?.({
			academicTermId,
			name: name.trim(),
			description: description.trim() || null,
			examKind
		});
		if (created) resetForm();
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="max-w-xl p-0">
		<Dialog.Header class="border-b px-6 py-5">
			<Dialog.Title>สร้างรอบตารางสอบ</Dialog.Title>
			<Dialog.Description>รอบสอบจะถูกสร้างในภาคเรียนที่เลือกบนแถบด้านบน</Dialog.Description>
		</Dialog.Header>

		<form
			class="space-y-5 px-6 py-5"
			onsubmit={(event) => {
				event.preventDefault();
				submitForm();
			}}
		>
			<div class="grid gap-2">
				<Label for="exam-round-name">ชื่อรอบสอบ</Label>
				<Input
					id="exam-round-name"
					bind:value={name}
					placeholder="เช่น กลางภาคเรียนที่ 1"
					maxlength={160}
					required
				/>
			</div>

			<div class="grid gap-2">
				<Label>ภาคเรียน</Label>
				<div class="bg-muted/40 rounded-md border px-3 py-2 text-sm">{termLabel}</div>
			</div>

			<div class="grid gap-2">
				<Label>ชนิดรอบสอบ</Label>
				<Select.Root type="single" bind:value={examKind}>
					<Select.Trigger class="w-full">
						{examKind === 'final' ? 'ปลายภาค' : 'กลางภาค'}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="midterm">กลางภาค</Select.Item>
						<Select.Item value="final">ปลายภาค</Select.Item>
					</Select.Content>
				</Select.Root>
			</div>

			<div class="grid gap-2">
				<Label for="exam-round-description">รายละเอียด</Label>
				<Textarea
					id="exam-round-description"
					bind:value={description}
					class="min-h-24"
					placeholder="เว้นว่างได้"
				/>
			</div>

			<Dialog.Footer class="border-t pt-4">
				<Button type="button" variant="outline" onclick={() => (open = false)}>ยกเลิก</Button>
				<LoadingButton
					type="submit"
					loading={saving}
					loadingLabel="กำลังสร้าง..."
					disabled={!name.trim() || !academicTermId}
				>
					สร้างรอบสอบ
				</LoadingButton>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
