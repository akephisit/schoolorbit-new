<script lang="ts">
	import { page } from '$app/state';
	import { ArrowLeft } from '@lucide/svelte';
	import {
		academicContextualMenuPath,
		readAcademicContextFromUrl
	} from '#lib/academic-context/route-context.js';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { provideSupervisionObservationWrites } from '#lib/supervision/observation-writes.js';
	import {
		supervisionDestinations,
		supervisionHomeLabel,
		supervisionHomePath,
		visibleSupervisionDestinations
	} from '#lib/supervision/workspace-navigation.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	provideSupervisionObservationWrites();
	const academicContext = getAcademicContextStore();
	const activePath = $derived(
		supervisionDestinations.find(({ path }) => `/(app)${path}` === page.route.id)?.path
	);
	const selectedContext = $derived(readAcademicContextFromUrl(new URL(page.url.href)));
	const visibleLinks = $derived(visibleSupervisionDestinations($authStore.user, $userPermissions));

	function sectionRoute(path: string) {
		return academicContextualMenuPath(path, selectedContext, $academicContext.options);
	}
</script>

{#if activePath}
	<div class="min-w-0 px-(--school-page-padding) pt-3 md:pt-(--school-page-padding)">
		<nav
			aria-label="เมนูนิเทศการสอน"
			data-testid="supervision-workflow-nav"
			class="flex flex-wrap items-center gap-1 md:rounded-xl md:border md:bg-card md:p-1.5"
		>
			<Button
				href={sectionRoute(supervisionHomePath)}
				data-sveltekit-preload-data={false}
				data-sveltekit-preload-code="hover"
				variant="ghost"
				size="sm"
				class="-ms-2 justify-start text-muted-foreground md:ms-0 md:me-1"
			>
				<ArrowLeft class="size-4" aria-hidden="true" />
				{supervisionHomeLabel}
			</Button>
			{#each visibleLinks as item (item.path)}
				{@const Icon = item.icon}
				<Button
					href={sectionRoute(item.path)}
					data-sveltekit-preload-data={false}
					data-sveltekit-preload-code="hover"
					variant="ghost"
					size="sm"
					aria-current={activePath === item.path ? 'page' : undefined}
					class="hidden min-w-0 justify-start border border-transparent text-muted-foreground md:inline-flex aria-[current=page]:border-primary/20 aria-[current=page]:bg-primary/10 aria-[current=page]:font-semibold aria-[current=page]:text-primary"
				>
					<Icon class="size-4" aria-hidden="true" />
					{item.label}
				</Button>
			{/each}
		</nav>
	</div>
{/if}

{@render children()}
