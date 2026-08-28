<script lang="ts">
	import type { CurriculumTermSlotInput } from '$lib/api/academic-core';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import { Plus, Trash2 } from 'lucide-svelte';

	let {
		slots,
		onchange
	}: {
		slots: CurriculumTermSlotInput[];
		onchange: (slots: CurriculumTermSlotInput[]) => void;
	} = $props();

	const typeLabels = {
		regular: 'ภาคเรียนปกติ',
		summer: 'ภาคฤดูร้อน',
		remedial: 'ภาคซ่อมเสริม',
		custom: 'ภาคเรียนกำหนดเอง'
	} as const;
	const termTypes: CurriculumTermSlotInput['termType'][] = [
		'regular',
		'summer',
		'remedial',
		'custom'
	];
	function isTermType(value: string): value is CurriculumTermSlotInput['termType'] {
		return termTypes.some((termType) => termType === value);
	}

	function update(index: number, values: Partial<CurriculumTermSlotInput>) {
		onchange(slots.map((slot, slotIndex) => (slotIndex === index ? { ...slot, ...values } : slot)));
	}

	function add(termType: CurriculumTermSlotInput['termType']) {
		const occurrence =
			Math.max(0, ...slots.filter((slot) => slot.termType === termType).map((slot) => slot.typeOccurrence)) +
			1;
		const name =
			termType === 'regular'
				? `ภาคเรียนที่ ${occurrence}`
				: termType === 'summer'
					? `ภาคฤดูร้อน ${occurrence}`
					: termType === 'remedial'
						? `ภาคซ่อมเสริม ${occurrence}`
						: `ภาคเรียนพิเศษ ${occurrence}`;
		onchange([
			...slots,
			{ sequence: slots.length + 1, termType, typeOccurrence: occurrence, name }
		]);
	}

	function remove(index: number) {
		onchange(
			slots
				.filter((_, slotIndex) => slotIndex !== index)
				.map((slot, slotIndex) => ({ ...slot, sequence: slotIndex + 1 }))
		);
	}
</script>

<section class="space-y-3 rounded-xl border bg-muted/20 p-3">
	<div>
		<h3 class="font-semibold">ภาคเรียนในโครงสร้างหลักสูตร</h3>
		<p class="text-xs text-muted-foreground">
			เป็นช่องของหลักสูตร ไม่ใช่ภาคเรียนจริงของปีใดปีหนึ่ง ระบบจะจับคู่ตามประเภทและลำดับ
		</p>
	</div>

	<div class="space-y-2">
		{#each slots as slot, index (slot.id ?? `${slot.termType}-${slot.typeOccurrence}`)}
			<div class="grid gap-2 rounded-lg border bg-background p-2 sm:grid-cols-[4rem_minmax(12rem,1fr)_11rem_5rem_2.5rem] sm:items-center">
				<Input
					type="number"
					min="1"
					value={slot.sequence}
					oninput={(event) => update(index, { sequence: Number(event.currentTarget.value) })}
					aria-label="ลำดับภาคเรียน"
				/>
				<Input
					value={slot.name}
					oninput={(event) => update(index, { name: event.currentTarget.value })}
					aria-label="ชื่อภาคเรียน"
				/>
				<Select.Root
					type="single"
					value={slot.termType}
					onValueChange={(value) => value && isTermType(value) && update(index, { termType: value })}
				>
					<Select.Trigger>{typeLabels[slot.termType]}</Select.Trigger>
					<Select.Content>
						{#each termTypes as value (value)}
							<Select.Item value={value}>{typeLabels[value]}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				<Input
					type="number"
					min="1"
					value={slot.typeOccurrence}
					oninput={(event) => update(index, { typeOccurrence: Number(event.currentTarget.value) })}
					aria-label="ลำดับภายในประเภท"
				/>
				<Button type="button" variant="ghost" size="icon" onclick={() => remove(index)} aria-label="ลบภาคเรียน">
					<Trash2 class="size-4" />
				</Button>
			</div>
		{/each}
	</div>

	<div class="flex flex-wrap gap-2">
		{#each termTypes as value (value)}
			<Button type="button" variant="outline" size="sm" onclick={() => add(value)}>
				<Plus class="size-3.5" /> {typeLabels[value]}
			</Button>
		{/each}
	</div>
</section>
