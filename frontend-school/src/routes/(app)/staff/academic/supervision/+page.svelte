<script lang="ts">
	import { page } from '$app/state';
	import { ChevronRight } from '@lucide/svelte';
	import {
		academicContextualMenuPath,
		readAcademicContextFromUrl
	} from '#lib/academic-context/route-context.js';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { visibleSupervisionDestinations } from '#lib/supervision/workspace-navigation.js';
	import { cn } from '#lib/utils.js';

	const academicContext = getAcademicContextStore();
	const selectedContext = $derived(readAcademicContextFromUrl(new URL(page.url.href)));
	const destinations = $derived(visibleSupervisionDestinations($authStore.user, $userPermissions));
	const groups = $derived(
		[
			{
				key: 'work',
				title: 'งานนิเทศ',
				destinations: destinations.filter(({ group }) => group === 'work')
			},
			{
				key: 'setup',
				title: 'ตั้งค่าการนิเทศ',
				destinations: destinations.filter(({ group }) => group === 'setup')
			}
		].filter(({ destinations }) => destinations.length > 0)
	);
</script>

<PageShell title="นิเทศการสอน">
	<nav aria-label="เมนูนิเทศการสอน" class="space-y-6" data-testid="supervision-hub">
		{#each groups as group (group.key)}
			<section aria-labelledby={`supervision-hub-${group.key}`} class="space-y-3">
				<h2 id={`supervision-hub-${group.key}`} class="text-sm font-semibold text-muted-foreground">
					{group.title}
				</h2>
				<ul
					class={cn(
						'grid gap-3',
						group.destinations.length > 1 && 'min-[360px]:grid-cols-2 lg:grid-cols-4',
						group.destinations.length === 1 && 'sm:max-w-sm'
					)}
				>
					{#each group.destinations as item (item.section)}
						{@const Icon = item.icon}
						<li class="min-w-0">
							<Button
								href={academicContextualMenuPath(
									item.path,
									selectedContext,
									$academicContext.options
								)}
								data-sveltekit-preload-data={false}
								data-sveltekit-preload-code="hover"
								aria-label={item.label}
								aria-describedby={`supervision-hub-description-${item.section}`}
								variant="outline"
								class={cn(
									'h-full min-h-28 w-full flex-col items-start justify-start gap-2.5 rounded-xl px-3 py-3 text-start whitespace-normal shadow-none sm:px-4 sm:py-4',
									item.section === 'mine' && 'border-primary/25 bg-primary/5 hover:bg-primary/10'
								)}
							>
								<span class="flex w-full items-center justify-between gap-2">
									<span
										class={cn(
											'inline-flex size-9 shrink-0 items-center justify-center rounded-lg bg-muted text-foreground',
											item.section === 'mine' && 'bg-primary/10 text-primary'
										)}
									>
										<Icon class="size-[18px]" aria-hidden="true" />
									</span>
									<ChevronRight class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
								</span>
								<span class="space-y-0.5">
									<span class="block text-base leading-snug font-semibold">{item.label}</span>
									<span
										id={`supervision-hub-description-${item.section}`}
										class="block text-sm leading-snug font-normal text-muted-foreground"
										>{item.description}</span
									>
								</span>
							</Button>
						</li>
					{/each}
				</ul>
			</section>
		{/each}
	</nav>
</PageShell>
