<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { BookOpen, LayoutGrid, School, UsersRound } from 'lucide-svelte';

	import type { TimetablePageView } from '$lib/academic/timetable/board-state';

	let {
		value,
		disabled = false,
		onViewChange
	}: {
		value: TimetablePageView;
		disabled?: boolean;
		onViewChange: (view: TimetablePageView) => void;
	} = $props();
</script>

<div class="flex flex-wrap rounded-lg border bg-muted/45 p-1" aria-label="มุมมองตารางสอน">
	<Button
		type="button"
		size="sm"
		variant={value === 'homeroom' ? 'default' : 'ghost'}
		class="h-8 rounded-md px-3"
		aria-pressed={value === 'homeroom'}
		{disabled}
		onclick={() => onViewChange('homeroom')}
	>
		<School class="size-3.5" />
		ห้องประจำชั้น
	</Button>
	<Button
		type="button"
		size="sm"
		variant={value === 'learning_group' ? 'default' : 'ghost'}
		class="h-8 rounded-md px-3"
		aria-pressed={value === 'learning_group'}
		{disabled}
		onclick={() => onViewChange('learning_group')}
	>
		<BookOpen class="size-3.5" />
		กลุ่มเรียน
	</Button>
	<Button
		type="button"
		size="sm"
		variant={value === 'teacher' ? 'default' : 'ghost'}
		class="h-8 rounded-md px-3"
		aria-pressed={value === 'teacher'}
		{disabled}
		onclick={() => onViewChange('teacher')}
	>
		<UsersRound class="size-3.5" />
		ครูผู้สอน
	</Button>
	<Button
		type="button"
		size="sm"
		variant={value === 'wholeSchool' ? 'default' : 'ghost'}
		class="h-8 rounded-md px-3"
		aria-pressed={value === 'wholeSchool'}
		{disabled}
		onclick={() => onViewChange('wholeSchool')}
	>
		<LayoutGrid class="size-3.5" />
		ทั้งโรงเรียน
	</Button>
</div>
