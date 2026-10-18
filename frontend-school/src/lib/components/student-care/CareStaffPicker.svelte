<script lang="ts">
	import type { CareStaffOption } from '#lib/api/student-care.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	let {
		staff,
		value = [],
		onchange,
		label = 'ผู้รับผิดชอบ',
		disabled = false
	}: {
		staff: CareStaffOption[];
		value?: string[];
		onchange: (ids: string[]) => void;
		label?: string;
		disabled?: boolean;
	} = $props();
	let search = $state('');
	const filtered = $derived(
		staff.filter((person) =>
			person.name.toLocaleLowerCase('th').includes(search.toLocaleLowerCase('th'))
		)
	);
</script>

<fieldset class="space-y-3" {disabled}>
	<legend class="text-sm font-medium">{label}</legend>
	{#if staff.length > 10}<Input
			aria-label={`ค้นหา${label}`}
			bind:value={search}
			placeholder="ค้นหาชื่อ"
		/>{/if}
	<div class="max-h-56 space-y-2 overflow-y-auto rounded-lg border p-3">
		{#each filtered as person (person.id)}
			<div class="flex items-center gap-2">
				<Checkbox
					id={`${label}-${person.id}`}
					checked={value.includes(person.id)}
					{disabled}
					onCheckedChange={(checked) =>
						onchange(
							checked
								? [...value.filter((id) => id !== person.id), person.id]
								: value.filter((id) => id !== person.id)
						)}
				/><Label for={`${label}-${person.id}`} class="min-w-0 break-words">{person.name}</Label>
			</div>
		{/each}
		{#if !filtered.length}<p class="text-sm text-muted-foreground">ไม่พบรายชื่อ</p>{/if}
	</div>
	<p class="text-xs text-muted-foreground">เลือกแล้ว {value.length} คน</p>
</fieldset>
