<script lang="ts">
	import { page } from '$app/state';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { Button } from '#lib/components/ui/button/index.js';
	const links = [
		{ href: '/staff/budget', label: 'ภาพรวม' },
		{ href: '/staff/budget/projects', label: 'โครงการ' },
		{ href: '/staff/budget/requests', label: 'คำขอใช้เงิน' },
		{ href: '/staff/budget/adjustments', label: 'ปรับงบประมาณ' },
		{ href: '/staff/budget/reports', label: 'รายงาน' }
	];
</script>

<nav class="flex flex-wrap gap-1 border-b pb-3" aria-label="งานงบประมาณ">
	{#each links as link}<Button
			href={link.href}
			variant={page.url.pathname === link.href ? 'secondary' : 'ghost'}
			size="sm"
			aria-current={page.url.pathname === link.href ? 'page' : undefined}>{link.label}</Button
		>{/each}{#if $can.has(PERMISSIONS.BUDGET_MANAGE_SCHOOL)}<Button
			href="/staff/budget/settings"
			variant={page.url.pathname === '/staff/budget/settings' ? 'secondary' : 'ghost'}
			size="sm">ตั้งค่า</Button
		>{/if}
</nav>
