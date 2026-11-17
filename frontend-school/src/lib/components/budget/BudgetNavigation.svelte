<script lang="ts">
	import { page } from '$app/state';
	import {
		Wallet,
		FolderOpen,
		ReceiptText,
		ArrowLeftRight,
		ChartNoAxesCombined,
		Settings2
	} from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { budgetReadPermissions } from '#lib/budget/access.js';
	import { destination } from '#lib/budget/presentation.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';

	let { fiscalYearId = '' }: { fiscalYearId?: string } = $props();
	let navigation = $state<HTMLElement>();
	$effect(() => {
		void page.url.pathname;
		if (!navigation) return;
		const selected = navigation.querySelector<HTMLElement>('[aria-current="page"]');
		if (!selected) return;
		const bounds = navigation.getBoundingClientRect();
		const activeBounds = selected.getBoundingClientRect();
		if (activeBounds.left < bounds.left)
			navigation.scrollLeft -= bounds.left - activeBounds.left + 6;
		else if (activeBounds.right > bounds.right)
			navigation.scrollLeft += activeBounds.right - bounds.right + 6;
	});
	const links = [
		{ path: '/staff/budget', label: 'ภาพรวม', icon: Wallet },
		{ path: '/staff/budget/projects', label: 'โครงการ', icon: FolderOpen },
		{ path: '/staff/budget/requests', label: 'คำขอใช้เงิน', icon: ReceiptText },
		{ path: '/staff/budget/adjustments', label: 'ปรับงบและเงินยืม', icon: ArrowLeftRight },
		{ path: '/staff/budget/reports', label: 'รายงาน', icon: ChartNoAxesCombined }
	];
	function active(path: string) {
		return (
			page.url.pathname === path ||
			(path !== '/staff/budget' && page.url.pathname.startsWith(`${path}/`))
		);
	}
</script>

{#if $can.hasAny(...budgetReadPermissions, PERMISSIONS.BUDGET_MANAGE_SCHOOL, PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL)}
	<nav
		bind:this={navigation}
		aria-label="งานงบประมาณ"
		class="min-w-0 overflow-x-auto rounded-xl border bg-card p-1.5"
	>
		<div class="flex w-max min-w-full items-center gap-1">
			{#if $can.hasAny(...budgetReadPermissions)}
				{#each links as link (link.path)}
					<Button
						href={destination(link.path, { fiscalYearId })}
						variant={active(link.path) ? 'secondary' : 'ghost'}
						size="sm"
						aria-current={active(link.path) ? 'page' : undefined}
						class="shrink-0"
					>
						<link.icon class="size-4" aria-hidden="true" />{link.label}
					</Button>
				{/each}
			{/if}
			{#if $can.hasAny(PERMISSIONS.BUDGET_MANAGE_SCHOOL, PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL)}
				<Button
					href={destination('/staff/budget/settings', { fiscalYearId })}
					variant={active('/staff/budget/settings') ? 'secondary' : 'ghost'}
					size="sm"
					aria-current={active('/staff/budget/settings') ? 'page' : undefined}
					class="shrink-0"><Settings2 class="size-4" aria-hidden="true" />ตั้งค่า</Button
				>
			{/if}
		</div>
	</nav>
{/if}
