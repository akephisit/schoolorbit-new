<script lang="ts">
	import { page } from '$app/state';
	import {
		BarChart3,
		CalendarDays,
		ClipboardCheck,
		ClipboardList,
		Inbox,
		UserRound
	} from '@lucide/svelte';
	import {
		academicContextualMenuPath,
		readAcademicContextFromUrl
	} from '#lib/academic-context/route-context.js';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import { userCanAccessRoute } from '#lib/auth/route-access.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { provideSupervisionObservationWrites } from '#lib/supervision/observation-writes.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	provideSupervisionObservationWrites();
	const academicContext = getAcademicContextStore();
	const basePath = '/staff/academic/supervision';
	const links = [
		{ path: basePath, label: 'ของฉัน', icon: UserRound },
		{ path: `${basePath}/evaluate`, label: 'ประเมิน', icon: ClipboardCheck },
		{ path: `${basePath}/requests`, label: 'คำขอจอง', icon: Inbox },
		{ path: `${basePath}/overview`, label: 'ภาพรวม', icon: BarChart3 },
		{ path: `${basePath}/cycles`, label: 'รอบนิเทศ', icon: CalendarDays },
		{ path: `${basePath}/templates`, label: 'แบบประเมิน', icon: ClipboardList }
	];
	const activePath = $derived(links.find(({ path }) => `/(app)${path}` === page.route.id)?.path);
	const selectedContext = $derived(readAcademicContextFromUrl(new URL(page.url.href)));
	const visibleLinks = $derived(
		links.filter(({ path }) =>
			userCanAccessRoute($authStore.user, $userPermissions, `/(app)${path}`)
		)
	);

	function sectionRoute(path: string) {
		return academicContextualMenuPath(path, selectedContext, $academicContext.options);
	}
</script>

{#if activePath}
	<div class="min-w-0 px-(--school-page-padding) pt-(--school-page-padding)">
		<nav
			aria-label="เมนูนิเทศการสอน"
			class="grid grid-cols-2 gap-1 rounded-xl border bg-card p-1.5 sm:flex sm:flex-wrap"
		>
			{#each visibleLinks as item (item.path)}
				{@const Icon = item.icon}
				<Button
					href={sectionRoute(item.path)}
					data-sveltekit-preload-data={false}
					data-sveltekit-preload-code="hover"
					variant="ghost"
					aria-current={activePath === item.path ? 'page' : undefined}
					class="h-auto min-w-0 justify-start border border-transparent px-3 py-2 text-start whitespace-normal text-muted-foreground aria-[current=page]:border-primary/20 aria-[current=page]:bg-primary/10 aria-[current=page]:font-semibold aria-[current=page]:text-primary"
				>
					<Icon class="size-4" aria-hidden="true" />
					{item.label}
				</Button>
			{/each}
		</nav>
	</div>
{/if}

{@render children()}
