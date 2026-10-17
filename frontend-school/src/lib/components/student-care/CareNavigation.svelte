<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { can } from '#lib/stores/permissions.js';

	import { careLink } from '#lib/features/student-care/presentation.js';
	import {
		careRoutineReadPermissions,
		careSensitiveReadPermissions,
		careSensitiveManagePermissions
	} from '#lib/features/student-care/access.js';
	let { term, current = '' }: { term?: string | null; current?: string } = $props();
</script>

<nav aria-label="งานดูแลช่วยเหลือนักเรียน" class="flex flex-wrap gap-2">
	<Button href={careLink('', term)} variant={current === '' ? 'secondary' : 'ghost'}>ภาพรวม</Button
	><Button href={careLink('/rounds', term)} variant={current === 'rounds' ? 'secondary' : 'ghost'}
		>รอบประเมิน</Button
	>{#if $can.hasAny(...careRoutineReadPermissions)}<Button
			href={careLink('/visits', term)}
			variant={current === 'visits' ? 'secondary' : 'ghost'}>เยี่ยมบ้าน</Button
		>{/if}{#if $can.hasAny(...careSensitiveReadPermissions, ...careSensitiveManagePermissions)}<Button
			href={careLink('/cases', term)}
			variant={current === 'cases' ? 'secondary' : 'ghost'}>การดูแลรายกรณี</Button
		>{/if}<Button
		href={careLink('/library', term)}
		variant={current === 'library' ? 'secondary' : 'ghost'}>คลังแบบประเมิน</Button
	>
</nav>
