<script lang="ts">
	import {
		nextEditableCell,
		normalizeScorePaste,
		type GradebookCellPosition,
		type ScorePasteMutation
	} from '$lib/academic/gradebook/ledger';
	import type { GradebookScoreItem, GroupPhaseWorkspace } from '$lib/api/academicGradebook';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { CheckCheck, Pencil, Plus, Smartphone } from 'lucide-svelte';

	let {
		workspace,
		values,
		selectedItemIds,
		canManage,
		disabled = false,
		onselectionchange,
		onmutations,
		onflush,
		onopenitem,
		onopenmobile,
		onconfirm,
		onerror
	}: {
		workspace: GroupPhaseWorkspace;
		values: Record<string, string | null>;
		selectedItemIds: string[];
		canManage: boolean;
		disabled?: boolean;
		onselectionchange: (ids: string[]) => void;
		onmutations: (mutations: ScorePasteMutation[]) => boolean;
		onflush: () => Promise<void>;
		onopenitem: (item: GradebookScoreItem | null) => void;
		onopenmobile: (position: GradebookCellPosition) => void;
		onconfirm: () => void;
		onerror: (message: string) => void;
	} = $props();

	let activeItems = $derived(
		workspace.items
			.filter((item) => item.lifecycle === 'active')
			.toSorted((left, right) => left.displayOrder - right.displayOrder)
	);
	let studentIds = $derived(workspace.students.map((student) => student.studentAcademicYearId));
	let activeMaximum = $derived(
		activeItems.reduce((total, item) => total + (Number(item.maxScore) || 0), 0)
	);
	let phaseMaximum = $derived(Number(workspace.phaseMaxScore) || 0);

	function key(studentId: string, itemId: string): string {
		return `${studentId}:${itemId}`;
	}

	function studentTotal(studentId: string): number {
		return activeItems.reduce(
			(total, item) => total + (Number(values[key(studentId, item.id)]) || 0),
			0
		);
	}

	function toggleItem(itemId: string, checked: boolean) {
		onselectionchange(
			checked
				? selectedItemIds.includes(itemId)
					? selectedItemIds
					: [...selectedItemIds, itemId]
				: selectedItemIds.filter((id) => id !== itemId)
		);
	}

	function cellId(position: GradebookCellPosition): string {
		return `score-${position.studentId}-${position.itemId}`;
	}

	function commitCell(position: GradebookCellPosition, rawValue: string): boolean {
		const result = normalizeScorePaste(rawValue, position, selectedItemIds, studentIds);
		if (!result.ok) {
			onerror(result.error.message);
			return false;
		}
		return onmutations(result.mutations);
	}

	function handleKeydown(event: KeyboardEvent, position: GradebookCellPosition) {
		if (event.key !== 'Tab' && event.key !== 'Enter') return;
		const input = event.currentTarget as HTMLInputElement;
		if (!commitCell(position, input.value)) {
			event.preventDefault();
			return;
		}
		event.preventDefault();
		const direction =
			event.key === 'Tab'
				? event.shiftKey
					? 'tab_backward'
					: 'tab_forward'
				: event.shiftKey
					? 'enter_up'
					: 'enter_down';
		const next = nextEditableCell(position, selectedItemIds, studentIds, direction);
		void onflush().then(() => {
			if (next) document.getElementById(cellId(next))?.focus();
		});
	}

	function handlePaste(event: ClipboardEvent, position: GradebookCellPosition) {
		const text = event.clipboardData?.getData('text/plain');
		if (text == null) return;
		event.preventDefault();
		const result = normalizeScorePaste(text, position, selectedItemIds, studentIds);
		if (!result.ok) {
			onerror(result.error.message);
			return;
		}
		onmutations(result.mutations);
	}
</script>

<section class="overflow-hidden rounded-xl border bg-card" aria-label="ตารางกรอกคะแนน">
	<header
		class="flex flex-col gap-3 border-b px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
	>
		<div>
			<div class="flex flex-wrap items-center gap-2">
				<h2 class="font-semibold">รายการคะแนนในช่วงนี้</h2>
				<Badge variant={activeMaximum === phaseMaximum ? 'secondary' : 'destructive'}>
					{activeMaximum.toLocaleString('th-TH')} / {phaseMaximum.toLocaleString('th-TH')} คะแนน
				</Badge>
			</div>
			<p class="mt-0.5 text-sm text-muted-foreground">
				ติ๊กหัวคอลัมน์ที่ต้องการกรอก ช่องที่ไม่ติ๊กยังนำมารวมคะแนนตามเดิม
			</p>
		</div>
		<div class="flex flex-wrap gap-2">
			<Button
				variant="outline"
				size="sm"
				disabled={disabled || !canManage || activeItems.length === 0}
				onclick={() => onselectionchange(activeItems.map((item) => item.id))}
			>
				<CheckCheck class="size-4" /> เลือกทุกช่อง
			</Button>
			<Button
				variant="outline"
				size="sm"
				disabled={disabled || !canManage || selectedItemIds.length === 0}
				onclick={() => onselectionchange([])}
			>
				ล้างการเลือก
			</Button>
			{#if canManage}
				<Button size="sm" {disabled} onclick={() => onopenitem(null)}>
					<Plus class="size-4" /> เพิ่มรายการคะแนน
				</Button>
			{/if}
		</div>
	</header>

	{#if activeItems.length === 0}
		<div class="flex min-h-44 flex-col items-center justify-center px-6 text-center">
			<p class="font-medium">ยังไม่มีรายการคะแนนในช่วงนี้</p>
			<p class="mt-1 text-sm text-muted-foreground">
				เพิ่มชีท งาน หรือส่วนของข้อสอบก่อนเริ่มกรอกคะแนน
			</p>
		</div>
	{:else}
		<div class="hidden overflow-x-auto md:block">
			<table class="min-w-max border-collapse text-sm">
				<thead>
					<tr class="border-b bg-muted/30">
						<th class="sticky left-0 z-20 w-12 min-w-12 bg-muted px-3 py-3 text-center font-medium"
							>ที่</th
						>
						<th
							class="sticky left-12 z-20 w-56 min-w-56 border-r bg-muted px-3 py-3 text-left font-medium"
							>นักเรียน</th
						>
						{#each activeItems as item (item.id)}
							<th
								class={[
									'w-28 min-w-28 border-r px-2 py-2 align-top',
									selectedItemIds.includes(item.id) &&
										'bg-primary/10 shadow-[inset_0_3px_0_hsl(var(--primary))]'
								]}
							>
								<div class="flex items-start justify-between gap-1">
									<label class="flex min-w-0 cursor-pointer items-start gap-2 text-left">
										<Checkbox
											checked={selectedItemIds.includes(item.id)}
											disabled={disabled || !canManage}
											onCheckedChange={(checked) => toggleItem(item.id, checked === true)}
										/>
										<span class="min-w-0">
											<span class="block truncate font-medium" title={item.name}>{item.name}</span>
											<span class="block text-xs font-normal text-muted-foreground"
												>เต็ม {item.maxScore}</span
											>
										</span>
									</label>
									{#if canManage}
										<Button
											variant="ghost"
											size="icon-sm"
											class="size-7"
											aria-label={`แก้ ${item.name}`}
											{disabled}
											onclick={() => onopenitem(item)}
										>
											<Pencil class="size-3.5" />
										</Button>
									{/if}
								</div>
							</th>
						{/each}
						<th class="w-24 min-w-24 px-3 py-3 text-right font-medium">รวมช่วงนี้</th>
					</tr>
				</thead>
				<tbody>
					{#each workspace.students as student, index (student.studentAcademicYearId)}
						<tr class="border-b last:border-b-0 hover:bg-muted/20">
							<td class="sticky left-0 z-10 bg-card px-3 py-2 text-center text-muted-foreground"
								>{index + 1}</td
							>
							<td class="sticky left-12 z-10 border-r bg-card px-3 py-2 font-medium"
								>{student.displayName}</td
							>
							{#each activeItems as item (item.id)}
								<td class={['border-r p-1.5', selectedItemIds.includes(item.id) && 'bg-primary/5']}>
									<Input
										id={cellId({ itemId: item.id, studentId: student.studentAcademicYearId })}
										class="mx-auto h-8 w-20 px-2 text-right tabular-nums"
										inputmode="decimal"
										value={values[key(student.studentAcademicYearId, item.id)] ?? ''}
										disabled={disabled || !canManage || !selectedItemIds.includes(item.id)}
										aria-label={`${item.name} ${student.displayName}`}
										onchange={(event) =>
											commitCell(
												{ itemId: item.id, studentId: student.studentAcademicYearId },
												(event.currentTarget as HTMLInputElement).value
											)}
										onkeydown={(event) =>
											handleKeydown(event, {
												itemId: item.id,
												studentId: student.studentAcademicYearId
											})}
										onpaste={(event) =>
											handlePaste(event, {
												itemId: item.id,
												studentId: student.studentAcademicYearId
											})}
										onblur={() => void onflush()}
									/>
								</td>
							{/each}
							<td class="px-3 py-2 text-right font-semibold tabular-nums"
								>{studentTotal(student.studentAcademicYearId).toLocaleString('th-TH')}</td
							>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>

		<div class="divide-y md:hidden">
			{#each workspace.students as student, index (student.studentAcademicYearId)}
				<div class="flex items-center gap-3 px-4 py-3">
					<span
						class="flex size-8 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-medium"
						>{index + 1}</span
					>
					<div class="min-w-0 flex-1">
						<p class="truncate font-medium">{student.displayName}</p>
						<p class="text-xs text-muted-foreground">
							รวม {studentTotal(student.studentAcademicYearId).toLocaleString('th-TH')} คะแนน
						</p>
					</div>
					<Button
						variant="outline"
						size="sm"
						disabled={disabled || !canManage || selectedItemIds.length === 0}
						onclick={() =>
							onopenmobile({
								itemId: selectedItemIds[0]!,
								studentId: student.studentAcademicYearId
							})}
					>
						<Smartphone class="size-4" /> กรอก
					</Button>
				</div>
			{/each}
		</div>
	{/if}

	<footer
		class="flex flex-col gap-2 border-t bg-muted/20 px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
	>
		<p class="text-sm text-muted-foreground">
			{workspace.confirmationIsCurrent
				? 'ยืนยันคะแนนช่วงนี้แล้ว'
				: 'คะแนนช่วงนี้ยังไม่ได้ยืนยันหรือมีการเปลี่ยนแปลง'}
		</p>
		<Button
			variant={workspace.confirmationIsCurrent ? 'outline' : 'default'}
			disabled={disabled || !workspace.canConfirm || activeMaximum !== phaseMaximum}
			onclick={onconfirm}
		>
			<CheckCheck class="size-4" />
			{workspace.confirmationIsCurrent ? 'ยืนยันอีกครั้ง' : 'ยืนยันคะแนนช่วงนี้'}
		</Button>
	</footer>
</section>
