<script lang="ts">
	import { CalendarDays, House, Inbox, LayoutGrid } from '@lucide/svelte';
	import { page } from '$app/state';
	import { Button } from '#lib/components/ui/button/index.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { userCanAccessRoute } from '#lib/auth/route-access.js';
	import { getAppMenuRegion } from '#lib/navigation/app-menu.svelte.js';
	import { menuPreloadPolicy } from '#lib/navigation/menu-preload.js';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import {
		academicContextualMenuPath,
		getAcademicContextOptionsSource,
		getAcademicContextRequirement,
		readAcademicContextFromUrl
	} from '#lib/academic-context/route-context.js';
	import { cn } from '#lib/utils.js';

	let {
		onServicesClick,
		servicesOpen = false,
		pendingPath = null
	}: {
		onServicesClick: () => void;
		servicesOpen?: boolean;
		pendingPath?: string | null;
	} = $props();

	const appMenu = getAppMenuRegion();
	const academicContext = getAcademicContextStore();
	const destinations = [
		{ path: '/staff', label: 'หน้าหลัก', icon: House },
		{ path: '/staff/work', label: 'งานของฉัน', icon: Inbox },
		{ path: '/staff/timetable', label: 'ตารางสอน', icon: CalendarDays }
	];
	const visibleDestinations = $derived(
		destinations.filter(
			(item) =>
				userCanAccessRoute($authStore.user, $userPermissions, `/(app)${item.path}`) &&
				(item.path !== '/staff/timetable' || !appMenu.loaded || !!configuredItem(item.path))
		)
	);
	const menuContext = $derived(
		getAcademicContextRequirement(page.route.id) === 'none'
			? $academicContext.selected
			: readAcademicContextFromUrl(new URL(page.url.href))
	);
	const selectedPath = $derived(pendingPath ?? page.url.pathname);
	const servicesSelected = $derived(
		!destinations.some((item) => isActive(item.path, selectedPath))
	);

	function isActive(path: string, currentPath: string): boolean {
		return currentPath === path || (path !== '/staff' && currentPath.startsWith(`${path}/`));
	}

	function configuredItem(path: string) {
		return appMenu.groups
			.flatMap((group) => group.items)
			.find((item) => new URL(item.path, page.url.href).pathname === path);
	}

	function destinationHref(path: string): string {
		const configured = configuredItem(path);
		return academicContextualMenuPath(
			configured?.path ?? path,
			menuContext,
			$academicContext.optionsBySource[getAcademicContextOptionsSource(`/(app)${path}`)]
		);
	}

	function controlClass(selected: boolean): string {
		return cn(
			'h-auto min-h-14 min-w-0 w-full flex-col gap-1 rounded-lg px-1 py-2 text-xs transition-none',
			selected
				? 'bg-secondary text-foreground font-semibold'
				: 'text-muted-foreground hover:text-foreground'
		);
	}
</script>

<nav
	aria-label="เมนูหลักบุคลากร"
	class="shrink-0 border-t border-border bg-card px-2 pt-1 pb-[max(0.25rem,env(safe-area-inset-bottom))] lg:hidden"
>
	<div class="grid grid-flow-col auto-cols-fr gap-1">
		{#each visibleDestinations as item (item.path)}
			{@const selected = isActive(item.path, selectedPath)}
			<Button
				href={destinationHref(item.path)}
				disabled={item.path === '/staff/timetable' && !appMenu.loaded}
				variant="ghost"
				class={controlClass(selected)}
				aria-current={isActive(item.path, page.url.pathname) ? 'page' : undefined}
				aria-busy={(pendingPath !== null && selected) || undefined}
				data-sveltekit-preload-data={menuPreloadPolicy(configuredItem(item.path) ?? item)}
			>
				<item.icon class="size-5" strokeWidth={selected ? 2.5 : 1.75} />
				<span>{item.label}</span>
			</Button>
		{/each}
		<Button
			variant="ghost"
			class={controlClass(servicesOpen || servicesSelected)}
			onclick={onServicesClick}
			aria-expanded={servicesOpen}
			aria-controls="app-sidebar"
			aria-haspopup="dialog"
		>
			<LayoutGrid class="size-5" strokeWidth={servicesOpen || servicesSelected ? 2.5 : 1.75} />
			<span>บริการ</span>
		</Button>
	</div>
</nav>
