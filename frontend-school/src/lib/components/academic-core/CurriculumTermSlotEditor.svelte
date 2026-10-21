<script lang="ts">
	import type { CurriculumTermSlotInput } from '#lib/api/academic-core.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Plus, Trash2 } from '@lucide/svelte';

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
			Math.max(
				0,
				...slots.filter((slot) => slot.termType === termType).map((slot) => slot.typeOccurrence)
			) + 1;
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
			{
				sequence: Math.max(0, ...slots.map((slot) => slot.sequence)) + 1,
				termType,
				typeOccurrence: occurrence,
				name
			}
		]);
	}

	function remove(index: number) {
		onchange(slots.filter((_, slotIndex) => slotIndex !== index));
	}
</script>

<section class="space-y-3 rounded-xl border bg-muted/20 p-3">
	<div>
		<h3 class="font-semibold">ภาคเรียนในโครงสร้างหลักสูตร</h3>
		<p class="text-xs text-muted-foreground">
			เป็นช่องของหลักสูตร ไม่ใช่ภาคเรียนจริงของปีใดปีหนึ่ง
			ระบบจับคู่กับภาคเรียนจริงตามประเภทและครั้งที่ภายในประเภทเดียวกัน เช่น ปกติครั้งที่ 2
			จับคู่ภาคเรียนปกติครั้งที่ 2 ของปีนั้น ชื่อและลำดับแสดงใช้เพื่อแสดงผลเท่านั้น
		</p>
	</div>

	<div class="space-y-2">
		{#each slots as slot, index (index)}
			<div
				class="grid gap-2 rounded-lg border bg-background p-2 lg:grid-cols-[5rem_minmax(10rem,1fr)_11rem_8rem_2.5rem] lg:items-end"
			>
				<div class="space-y-1">
					<Label for={`term-sequence-${index}`}>ลำดับแสดง</Label>
					<Input
						id={`term-sequence-${index}`}
						type="number"
						min="1"
						value={slot.sequence}
						oninput={(event) => update(index, { sequence: Number(event.currentTarget.value) })}
						aria-label="ลำดับแสดง"
					/>
				</div>
				<div class="space-y-1">
					<Label for={`term-name-${index}`}>ชื่อภาคเรียน</Label>
					<Input
						id={`term-name-${index}`}
						value={slot.name}
						oninput={(event) => update(index, { name: event.currentTarget.value })}
						aria-label="ชื่อภาคเรียน"
					/>
				</div>
				<div class="space-y-1">
					<Label for={`term-type-${index}`}>ประเภท</Label>
					<Select.Root
						type="single"
						value={slot.termType}
						onValueChange={(value) =>
							value && isTermType(value) && update(index, { termType: value })}
					>
						<Select.Trigger id={`term-type-${index}`} aria-label="ประเภทภาคเรียน"
							>{typeLabels[slot.termType]}</Select.Trigger
						>
						<Select.Content>
							{#each termTypes as value (value)}
								<Select.Item {value}>{typeLabels[value]}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="space-y-1">
					<Label for={`term-occurrence-${index}`}>ครั้งที่ภายในประเภท</Label>
					<Input
						id={`term-occurrence-${index}`}
						type="number"
						min="1"
						value={slot.typeOccurrence}
						oninput={(event) =>
							update(index, { typeOccurrence: Number(event.currentTarget.value) })}
						aria-label="ครั้งที่ภายในประเภท"
					/>
				</div>
				<Button
					type="button"
					variant="ghost"
					size="icon"
					onclick={() => remove(index)}
					aria-label="ลบภาคเรียน"
				>
					<Trash2 class="size-4" />
				</Button>
			</div>
		{/each}
	</div>

	<div class="flex flex-wrap gap-2">
		{#each termTypes as value (value)}
			<Button type="button" variant="outline" size="sm" onclick={() => add(value)}>
				<Plus class="size-3.5" />
				{typeLabels[value]}
			</Button>
		{/each}
	</div>
</section>
