<script lang="ts">
	import { Button } from '$lib/components/ui/button';

	type InstructorOption = {
		id: string;
		displayName: string;
		role: 'primary' | 'secondary' | 'assistant';
	};

	let {
		options,
		value = $bindable<string[]>([]),
		disabled = false,
		label = 'ครูผู้สอนของคาบนี้'
	}: {
		options: InstructorOption[];
		value?: string[];
		disabled?: boolean;
		label?: string;
	} = $props();

	const roleLabels: Record<InstructorOption['role'], string> = {
		primary: 'ครูหลัก',
		secondary: 'ครูร่วมสอน',
		assistant: 'ครูผู้ช่วย'
	};

	function toggleInstructor(id: string): void {
		if (disabled) return;
		const nextIds = value.includes(id) ? value.filter((item) => item !== id) : [...value, id];
		const selected = new Set(nextIds);
		value = options.filter((option) => selected.has(option.id)).map((option) => option.id);
	}
</script>

<div class="space-y-2">
	<p class="text-sm font-medium">{label}</p>
	{#if options.length === 0}
		<div
			class="rounded-lg border border-dashed bg-muted/25 px-3 py-3 text-sm text-muted-foreground"
		>
			ยังไม่มีครูผู้สอนที่เลือกได้ กรุณากำหนดครูและช่วงวันที่ในหน้าจัดการกลุ่มเรียนก่อน
		</div>
	{:else}
		<div class="flex flex-wrap gap-2" aria-label={label}>
			{#each options as option (option.id)}
				{@const selected = value.includes(option.id)}
				<Button
					type="button"
					size="sm"
					variant={selected ? 'default' : 'outline'}
					class="h-auto min-h-9 gap-2 rounded-full px-3 py-1.5"
					aria-pressed={selected}
					{disabled}
					onclick={() => toggleInstructor(option.id)}
				>
					<span>{option.displayName}</span>
					<span
						class={selected
							? 'text-primary-foreground/75 text-[0.7rem]'
							: 'text-muted-foreground text-[0.7rem]'}>{roleLabels[option.role]}</span
					>
				</Button>
			{/each}
		</div>
		<p class="text-xs text-muted-foreground">
			เลือกได้มากกว่าหนึ่งคน ระบบจะนับการชนและภาระงานตามรายชื่อที่เลือกจริงในคาบนี้
		</p>
	{/if}
</div>
