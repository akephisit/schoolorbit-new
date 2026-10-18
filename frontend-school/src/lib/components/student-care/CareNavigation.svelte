<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import {
		LayoutDashboard,
		Inbox,
		ClipboardList,
		House,
		HeartHandshake,
		LibraryBig
	} from '@lucide/svelte';

	import { careLink } from '#lib/features/student-care/presentation.js';
	import {
		careRoutineReadPermissions,
		careSensitiveReadPermissions,
		careSensitiveManagePermissions
	} from '#lib/features/student-care/access.js';
	let { term, current = '' }: { term?: string | null; current?: string } = $props();
</script>

<nav
	aria-label="งานดูแลช่วยเหลือนักเรียน"
	class="flex flex-wrap gap-1 rounded-xl border bg-card p-2"
>
	<Button
		href={careLink('', term)}
		variant={current === '' ? 'secondary' : 'ghost'}
		aria-current={current === '' ? 'page' : undefined}
		><LayoutDashboard aria-hidden="true" />ภาพรวม</Button
	>{#if $can.has(PERMISSIONS.STUDENT_CARE_READ_OWN)}<Button
			href={careLink('/assignments', term)}
			variant={current === 'assignments' ? 'secondary' : 'ghost'}
			aria-current={current === 'assignments' ? 'page' : undefined}
			><Inbox aria-hidden="true" />แบบประเมินของฉัน</Button
		>{/if}<Button
		href={careLink('/rounds', term)}
		variant={current === 'rounds' ? 'secondary' : 'ghost'}
		aria-current={current === 'rounds' ? 'page' : undefined}
		><ClipboardList aria-hidden="true" />รอบประเมิน</Button
	>{#if $can.hasAny(...careRoutineReadPermissions)}<Button
			href={careLink('/visits', term)}
			variant={current === 'visits' ? 'secondary' : 'ghost'}
			aria-current={current === 'visits' ? 'page' : undefined}
			><House aria-hidden="true" />เยี่ยมบ้าน</Button
		>{/if}{#if $can.hasAny(...careSensitiveReadPermissions, ...careSensitiveManagePermissions)}<Button
			href={careLink('/cases', term)}
			variant={current === 'cases' ? 'secondary' : 'ghost'}
			aria-current={current === 'cases' ? 'page' : undefined}
			><HeartHandshake aria-hidden="true" />การดูแลรายกรณี</Button
		>{/if}<Button
		href={careLink('/library', term)}
		variant={current === 'library' ? 'secondary' : 'ghost'}
		aria-current={current === 'library' ? 'page' : undefined}
		><LibraryBig aria-hidden="true" />คลังแบบประเมิน</Button
	>
</nav>
