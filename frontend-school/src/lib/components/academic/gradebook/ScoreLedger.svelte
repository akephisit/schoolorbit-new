<script lang="ts">
	import { scoreItemBudget } from '$lib/academic/gradebook/item-budget';
	import {
		nextEditableCell,
		normalizeScorePaste,
		type GradebookCellPosition,
		type ScorePasteMutation
	} from '$lib/academic/gradebook/ledger';
	import type {
		GradebookPhaseCode,
		GradebookScoreItem,
		GroupPhaseWorkspace
	} from '$lib/api/academicGradebook';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { CheckCheck, Pencil, Plus, Smartphone } from 'lucide-svelte';

	let {
		workspaces,
		values,
		selectedItemIds,
		disabled = false,
		onselectionchange,
		onmutations,
		onflush,
		onopenitem,
		onopenmobile,
		onconfirm,
		onerror
	}: {
		workspaces: GroupPhaseWorkspace[];
		values: Record<string, string | null>;
		selectedItemIds: string[];
		disabled?: boolean;
		onselectionchange: (ids: string[]) => void;
		onmutations: (mutations: ScorePasteMutation[]) => boolean;
		onflush: () => Promise<void>;
		onopenitem: (phase: GradebookPhaseCode, item: GradebookScoreItem | null) => void;
		onopenmobile: (position: GradebookCellPosition) => void;
		onconfirm: (phase: GradebookPhaseCode) => void;
		onerror: (message: string) => void;
	} = $props();

	const phaseCodes: GradebookPhaseCode[] = ['before_midterm', 'midterm', 'after_midterm', 'final'];
	const phaseLabels = {
		before_midterm: 'ก่อนกลางภาค',
		midterm: 'กลางภาค',
		after_midterm: 'หลังกลางภาค',
		final: 'ปลายภาค'
	};
	let phases = $derived(
		phaseCodes.map((code) => {
			const workspace = workspaces.find((row) => row.phaseCode === code);
			const items = (workspace?.items ?? [])
				.filter((item) => item.lifecycle === 'active')
				.toSorted((a, b) => a.displayOrder - b.displayOrder);
			return {
				code,
				workspace,
				items,
				maximum: Number(workspace?.phaseMaxScore ?? 0),
				itemMaximum: scoreItemBudget(workspace?.phaseMaxScore ?? '0', items).allocated,
				budget: scoreItemBudget(workspace?.phaseMaxScore ?? '0', items),
				editable: Boolean(workspace?.canManage && !workspace.locked)
			};
		})
	);
	let students = $derived(workspaces[0]?.students ?? []);
	let studentIds = $derived(students.map((row) => row.studentAcademicYearId));
	let editableItems = $derived(
		phases.filter((phase) => phase.editable).flatMap((phase) => phase.items)
	);
	let editableIds = $derived(
		editableItems.filter((item) => selectedItemIds.includes(item.id)).map((item) => item.id)
	);
	let totalMaximum = $derived(phases.reduce((sum, phase) => sum + phase.maximum, 0));

	function key(studentId: string, itemId: string) {
		return studentId + ':' + itemId;
	}
	function total(studentId: string, items: GradebookScoreItem[]): number {
		return items.reduce((sum, item) => sum + Number(values[key(studentId, item.id)] ?? 0), 0);
	}
	function toggleItem(itemId: string, checked: boolean) {
		onselectionchange(
			checked
				? [...new Set([...selectedItemIds, itemId])]
				: selectedItemIds.filter((id) => id !== itemId)
		);
	}
	function cellId(position: GradebookCellPosition) {
		return 'score-' + position.studentId + '-' + position.itemId;
	}
	function commitCell(position: GradebookCellPosition, rawValue: string): boolean {
		const result = normalizeScorePaste(rawValue, position, editableIds, studentIds);
		if (!result.ok) {
			onerror(result.error.message);
			return false;
		}
		return onmutations(result.mutations);
	}
	async function handleKeydown(event: KeyboardEvent, position: GradebookCellPosition) {
		if (event.key !== 'Tab' && event.key !== 'Enter') return;
		event.preventDefault();
		if (!commitCell(position, (event.currentTarget as HTMLInputElement).value)) return;
		const direction =
			event.key === 'Tab'
				? event.shiftKey
					? 'tab_backward'
					: 'tab_forward'
				: event.shiftKey
					? 'enter_up'
					: 'enter_down';
		const next = nextEditableCell(position, editableIds, studentIds, direction);
		try {
			await onflush();
			if (next) document.getElementById(cellId(next))?.focus();
		} catch (error) {
			onerror(error instanceof Error ? error.message : 'บันทึกคะแนนไม่สำเร็จ');
		}
	}
	function handlePaste(event: ClipboardEvent, position: GradebookCellPosition) {
		const text = event.clipboardData?.getData('text/plain');
		if (text == null) return;
		event.preventDefault();
		const result = normalizeScorePaste(text, position, editableIds, studentIds);
		if (!result.ok) {
			onerror(result.error.message);
			return;
		}
		onmutations(result.mutations);
	}
</script>

<section class="overflow-hidden rounded-xl border bg-card" aria-label="ตารางกรอกคะแนน">
	<header class="flex flex-wrap items-center justify-between gap-3 border-b px-4 py-3">
		<div>
			<h2 class="font-semibold">คะแนนทั้งภาคเรียน</h2>
			<p class="text-sm text-muted-foreground">
				ติ๊กหัวคอลัมน์ที่ต้องการกรอก ช่องที่ไม่ติ๊กยังนำมารวมคะแนนตามเดิม
			</p>
		</div>
		<div class="flex gap-2">
			<Button
				variant="outline"
				size="sm"
				disabled={disabled || editableItems.length === 0}
				onclick={() => onselectionchange(editableItems.map((item) => item.id))}
				><CheckCheck class="size-4" /> เลือกทุกช่อง</Button
			>
			<Button
				variant="outline"
				size="sm"
				disabled={disabled || selectedItemIds.length === 0}
				onclick={() => onselectionchange([])}>ล้างการเลือก</Button
			>
		</div>
	</header>
	<div class="overflow-x-auto">
		<table class="w-full min-w-max border-collapse text-sm">
			<caption class="sr-only">คะแนนย่อยและคะแนนรวมทั้ง 4 ช่วงของภาคเรียน</caption>
			<thead>
				<tr class="border-b bg-muted/30">
					<th
						rowspan="2"
						scope="col"
						class="sticky left-0 z-20 w-12 min-w-12 bg-muted px-3 text-center">ที่</th
					>
					<th
						rowspan="2"
						scope="col"
						class="sticky left-12 z-20 w-40 min-w-40 border-r bg-muted px-3 text-left md:w-56 md:min-w-56"
						>นักเรียน</th
					>
					{#each phases as phase (phase.code)}
						<th
							scope="colgroup"
							colspan={phase.items.length + 1}
							class="border-r-2 px-3 py-3 text-left align-top"
						>
							<div class="flex items-center justify-between gap-3">
								<span>{phaseLabels[phase.code]}</span>
								{#if phase.editable}<Button
										variant="ghost"
										size="icon-sm"
										disabled={disabled || !phase.budget.canAdd}
										aria-label={'เพิ่มรายการคะแนน' + phaseLabels[phase.code]}
										onclick={() => onopenitem(phase.code, null)}><Plus class="size-4" /></Button
									>{/if}
							</div>
							{#if phase.workspace}
								<Badge variant={phase.itemMaximum === phase.maximum ? 'secondary' : 'destructive'}
									>{phase.itemMaximum} / {phase.maximum} คะแนน</Badge
								>
								<p class="mt-1 text-xs font-normal text-muted-foreground">
									{phase.budget.remaining === 0
										? 'จัดสรรคะแนนครบแล้ว'
										: phase.budget.remaining > 0
											? `เหลือจัดสรร ${phase.budget.remaining} คะแนน`
											: `เกิน ${-phase.budget.remaining} คะแนน กรุณาลดคะแนนเต็มของรายการ`}
								</p>
								{#if !phase.editable}<span class="ml-2 text-xs font-normal text-muted-foreground"
										>อ่านอย่างเดียว</span
									>{/if}
							{:else}<span class="text-xs font-normal text-muted-foreground"
									>ยังไม่มีโครงสร้างคะแนน</span
								>{/if}
						</th>
					{/each}
					<th rowspan="2" scope="col" class="min-w-24 bg-primary/5 px-3 text-right"
						>รวมทั้งหมด<span class="block text-xs font-normal text-muted-foreground"
							>เต็ม {totalMaximum}</span
						></th
					>
				</tr>
				<tr class="border-b bg-muted/20">
					{#each phases as phase (phase.code)}
						{#each phase.items as item (item.id)}
							<th
								scope="col"
								class={[
									'w-32 min-w-32 border-r px-2 py-2 align-top',
									editableIds.includes(item.id) && 'bg-primary/10'
								]}
							>
								<div class="flex items-start justify-between gap-1">
									<label class="flex min-w-0 items-start gap-2 text-left">
										<Checkbox
											checked={selectedItemIds.includes(item.id)}
											disabled={disabled || !phase.editable}
											onCheckedChange={(checked) => toggleItem(item.id, checked === true)}
											aria-label={'เลือก ' + phaseLabels[phase.code] + ' ' + item.name}
										/>
										<span
											><span class="block font-medium">{item.name}</span><span
												class="block text-xs font-normal text-muted-foreground"
												>เต็ม {item.maxScore}</span
											></span
										>
									</label>
									{#if phase.editable}<Button
											variant="ghost"
											size="icon-sm"
											class="size-7 shrink-0"
											aria-label={'แก้ ' + phaseLabels[phase.code] + ' ' + item.name}
											{disabled}
											onclick={() => onopenitem(phase.code, item)}
											><Pencil class="size-3.5" /></Button
										>{/if}
								</div>
							</th>
						{/each}
						<th scope="col" class="min-w-24 border-r-2 bg-primary/5 px-3 py-2 text-right"
							>รวม<span class="block text-xs font-normal text-muted-foreground"
								>เต็ม {phase.maximum}</span
							></th
						>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#each students as student, index (student.studentAcademicYearId)}
					<tr class="border-b last:border-b-0 hover:bg-muted/20">
						<td class="sticky left-0 z-10 bg-card px-3 py-2 text-center text-muted-foreground"
							>{index + 1}</td
						>
						<th
							scope="row"
							class="sticky left-12 z-10 border-r bg-card px-3 py-2 text-left font-medium"
							>{student.displayName}</th
						>
						{#each phases as phase (phase.code)}
							{#each phase.items as item (item.id)}
								{@const position = { itemId: item.id, studentId: student.studentAcademicYearId }}
								<td class={['border-r p-1.5', editableIds.includes(item.id) && 'bg-primary/5']}>
									<Input
										id={cellId(position)}
										class="mx-auto h-9 w-24 px-2 text-right tabular-nums"
										inputmode="decimal"
										value={values[key(student.studentAcademicYearId, item.id)] ?? ''}
										disabled={disabled || !editableIds.includes(item.id)}
										aria-label={phaseLabels[phase.code] +
											' ' +
											item.name +
											' ' +
											student.displayName}
										onchange={(event) => commitCell(position, event.currentTarget.value)}
										onkeydown={(event) => void handleKeydown(event, position)}
										onpaste={(event) => handlePaste(event, position)}
									/>
									{#if phase.editable}<Button
											variant="ghost"
											size="sm"
											class="mt-1 w-full md:hidden"
											disabled={disabled || !editableIds.includes(item.id)}
											aria-label={'เปิดกรอก ' +
												phaseLabels[phase.code] +
												' ' +
												item.name +
												' ' +
												student.displayName}
											onclick={() => onopenmobile(position)}
											><Smartphone class="size-3.5" /> กรอก</Button
										>{/if}
								</td>
							{/each}
							<td class="border-r-2 bg-primary/5 px-3 py-2 text-right font-semibold tabular-nums"
								>{phase.workspace
									? total(student.studentAcademicYearId, phase.items).toLocaleString('th-TH')
									: '—'}</td
							>
						{/each}
						<td class="bg-primary/5 px-3 py-2 text-right font-semibold tabular-nums"
							>{phases
								.reduce((sum, phase) => sum + total(student.studentAcademicYearId, phase.items), 0)
								.toLocaleString('th-TH')}</td
						>
					</tr>
				{:else}
					<tr
						><td
							colspan={3 + phases.reduce((sum, phase) => sum + phase.items.length + 1, 0)}
							class="p-6 text-center text-muted-foreground">ยังไม่มีนักเรียนในกลุ่มเรียนนี้</td
						></tr
					>
				{/each}
			</tbody>
			<tfoot>
				<tr class="border-t bg-muted/20">
					<th
						colspan="2"
						class="sticky left-0 z-10 border-r bg-muted px-3 py-3 text-left font-medium"
						>ยืนยันแยกแต่ละช่วง</th
					>
					{#each phases as phase (phase.code)}
						<td colspan={phase.items.length + 1} class="border-r-2 px-3 py-3 align-top">
							{#if phase.workspace}
								<p class="mb-2 text-xs text-muted-foreground">
									{phase.workspace.confirmationIsCurrent
										? 'ยืนยันแล้ว'
										: 'ยังไม่ยืนยัน / มีการเปลี่ยนแปลง'}
								</p>
								<Button
									size="sm"
									variant="outline"
									disabled={disabled ||
										phase.workspace.locked ||
										!phase.workspace.canConfirm ||
										phase.itemMaximum !== phase.maximum}
									onclick={() => onconfirm(phase.code)}
									><CheckCheck class="size-4" /> ยืนยัน{phaseLabels[phase.code]}</Button
								>
							{/if}
						</td>
					{/each}
					<td></td>
				</tr>
			</tfoot>
		</table>
	</div>
</section>
