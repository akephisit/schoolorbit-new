<script lang="ts">
	import type {
		LearnerEvaluationConfiguration,
		LearnerEvaluationCriterion
	} from '$lib/api/academicLearnerEvaluations';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Pencil, Plus, Trash2, X } from 'lucide-svelte';

	let {
		open,
		configuration,
		busy = false,
		onopenchange,
		oncreate,
		onupdate,
		onremove
	}: {
		open: boolean;
		configuration: LearnerEvaluationConfiguration | null;
		busy?: boolean;
		onopenchange: (open: boolean) => void;
		oncreate: (name: string) => void;
		onupdate: (criterion: LearnerEvaluationCriterion, name: string) => void;
		onremove: (criterion: LearnerEvaluationCriterion) => void;
	} = $props();

	let newName = $state('');
	let editingId = $state('');
	let editingName = $state('');

	function beginEdit(criterion: LearnerEvaluationCriterion) {
		editingId = criterion.id;
		editingName = criterion.name;
	}

	function create(event: SubmitEvent) {
		event.preventDefault();
		if (!newName.trim()) return;
		oncreate(newName.trim());
		newName = '';
	}

	function update(criterion: LearnerEvaluationCriterion) {
		if (!editingName.trim()) return;
		onupdate(criterion, editingName.trim());
		editingId = '';
	}
</script>

<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-xl">
		<Dialog.Header>
			<Dialog.Title>หัวข้อประเมินของรายวิชา</Dialog.Title>
			<Dialog.Description
				>หัวข้อชุดนี้ใช้กับทุกกลุ่มเรียนของรหัสวิชาในภาคเรียนเดียวกัน</Dialog.Description
			>
		</Dialog.Header>

		{#if configuration}
			<div class="divide-y rounded-lg border">
				{#each configuration.criteria.toSorted((left, right) => left.displayOrder - right.displayOrder) as criterion (criterion.id)}
					<div class="flex items-center gap-2 px-3 py-2.5">
						{#if editingId === criterion.id}
							<Input
								class="h-8 flex-1"
								bind:value={editingName}
								aria-label={`แก้ชื่อ ${criterion.name}`}
							/>
							<LoadingButton size="sm" loading={busy} onclick={() => update(criterion)}
								>บันทึก</LoadingButton
							>
							<Button
								variant="ghost"
								size="icon-sm"
								onclick={() => (editingId = '')}
								aria-label="ยกเลิกแก้ชื่อ"><X class="size-4" /></Button
							>
						{:else}
							<span
								class="flex size-7 shrink-0 items-center justify-center rounded-md bg-muted text-xs"
								>{criterion.displayOrder}</span
							>
							<p class="min-w-0 flex-1 text-sm font-medium">{criterion.name}</p>
							<Button
								variant="ghost"
								size="icon-sm"
								disabled={busy || configuration.locked}
								onclick={() => beginEdit(criterion)}
								aria-label={`แก้ ${criterion.name}`}><Pencil class="size-4" /></Button
							>
							<Button
								variant="ghost"
								size="icon-sm"
								class="text-destructive"
								disabled={busy || configuration.locked}
								onclick={() => onremove(criterion)}
								aria-label={`นำ ${criterion.name} ออก`}><Trash2 class="size-4" /></Button
							>
						{/if}
					</div>
				{/each}
			</div>

			<form class="space-y-2 rounded-lg border border-dashed p-3" onsubmit={create}>
				<Label for="new-evaluation-criterion">เพิ่มหัวข้อเฉพาะรายวิชา</Label>
				<div class="flex gap-2">
					<Input
						id="new-evaluation-criterion"
						bind:value={newName}
						placeholder="ชื่อหัวข้อประเมิน"
						disabled={configuration.locked}
					/>
					<LoadingButton
						type="submit"
						loading={busy}
						disabled={configuration.locked || !newName.trim()}
						><Plus class="size-4" /> เพิ่ม</LoadingButton
					>
				</div>
			</form>
		{:else}
			<div class="h-40 animate-pulse rounded-lg bg-muted"></div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
