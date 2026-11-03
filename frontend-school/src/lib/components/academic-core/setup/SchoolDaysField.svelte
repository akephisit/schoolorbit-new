<script lang="ts">
	import { normalizeSchoolDays, type AcademicWeekday } from '#lib/academic-core/foundation-presentation.js';
	import { SCHOOL_WEEKDAYS } from '#lib/academic-core/school-schedule.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	let { value, disabled = false, onchange, label = 'วันเรียนของภาคเรียน' }: {
		value: readonly string[];
		disabled?: boolean;
		onchange: (days: AcademicWeekday[]) => void;
		label?: string;
	} = $props();
</script>

<fieldset class="space-y-2" {disabled}>
	<legend class="text-sm font-medium">{label}</legend>
	<div class="flex flex-wrap gap-2">
		{#each SCHOOL_WEEKDAYS as day (day.code)}
			<label class="flex cursor-pointer items-center gap-2 rounded-lg border px-3 py-2 text-sm">
				<Checkbox checked={value.includes(day.code)} aria-label={`เลือกวัน${day.label}`}
					onCheckedChange={(checked) => onchange(normalizeSchoolDays(checked ? [...value, day.code] : value.filter((item) => item !== day.code)))} />
				{day.label}
			</label>
		{/each}
	</div>
</fieldset>
