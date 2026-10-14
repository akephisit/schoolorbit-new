<script lang="ts">
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { Button } from '#lib/components/ui/button/index.js';
	import { ListChecks, CalendarCheck, Settings2 } from '@lucide/svelte';
	let { term, active }: { term: string | null; active: 'items' | 'attendance' | 'policy' } =
		$props();
	const sections = [
		{ key: 'items', label: 'รายการพฤติกรรม', suffix: '/items', icon: ListChecks },
		{ key: 'attendance', label: 'คะแนนจากเช็กชื่อ', suffix: '/attendance', icon: CalendarCheck },
		{ key: 'policy', label: 'นโยบายและรอบรีเซ็ต', suffix: '', icon: Settings2 }
	];
	function href(suffix: string) {
		const query = new SvelteURLSearchParams();
		if (term) query.set('academicTermId', term);
		return `/staff/behavior/settings${suffix}?${query}`;
	}
</script>

<nav
	aria-label="หมวดการตั้งค่าพฤติกรรม"
	class="flex flex-wrap gap-2 rounded-xl border bg-card p-3 sm:p-4"
>
	{#each sections as section (section.key)}
		<Button
			href={href(section.suffix)}
			variant={active === section.key ? 'default' : 'ghost'}
			aria-current={active === section.key ? 'page' : undefined}
			class="min-h-[44px] min-w-[44px] gap-2"
		>
			<section.icon class="size-4 shrink-0" aria-hidden="true" />{section.label}
		</Button>
	{/each}
</nav>
