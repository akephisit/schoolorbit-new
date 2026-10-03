<script lang="ts">
	import type { PublicOrganizationNode } from '$lib/school-public/organization';
	import { groupPublicOrganizationMembers } from '$lib/school-public/organization';
	import { Building2, UserRound } from '@lucide/svelte';
	let { nodes }: { nodes: PublicOrganizationNode[] } = $props();
</script>

{#snippet branch(items: PublicOrganizationNode[], depth: number)}
	<ul
		class={depth === 0
			? 'grid gap-4'
			: depth === 1
				? 'mt-4 grid gap-3 border-l border-border pl-3 sm:pl-5 lg:grid-cols-2'
				: 'mt-4 grid gap-3 border-l border-border pl-3 sm:pl-5'}
	>
		{#each items as unit (unit.id)}
			<li class="min-w-0">
				<details
					open={depth < 2}
					class="public-surface rounded-xl border border-border bg-card p-4 sm:p-5"
				>
					<summary class="cursor-pointer text-base font-medium marker:text-primary">
						<Building2 class="mr-2 inline size-4 text-primary" />{unit.name}
						{#if unit.members.length}<span class="ml-2 text-xs font-normal text-muted-foreground"
								>{unit.members.length} คน</span
							>{/if}
						{#if unit.children.length}<span class="ml-2 text-xs font-normal text-muted-foreground"
								>{unit.children.length} หน่วยงานย่อย</span
							>{/if}
					</summary>
					<div class="mt-4 space-y-4">
						{#each groupPublicOrganizationMembers(unit.members) as group (group.code)}
							<section aria-label={group.label} class="rounded-lg bg-muted/35 p-3">
								<h4
									class="mb-3 flex items-center justify-between gap-2 text-xs font-medium text-primary"
								>
									{group.label}<span class="font-normal text-muted-foreground"
										>{group.members.length} คน</span
									>
								</h4>
								<ul class="space-y-3">
									{#each group.members as member, index (index)}
										<li class="flex items-start gap-3">
											<span
												class="flex size-8 shrink-0 items-center justify-center rounded-full bg-primary/10 text-primary"
												><UserRound class="size-4" /></span
											>
											<div class="min-w-0">
												<p class="break-words text-sm font-medium">{member.name}</p>
												{#if member.positionTitle}<p class="mt-1 text-xs text-muted-foreground">
														{member.positionTitle}
													</p>{/if}
											</div>
										</li>
									{/each}
								</ul>
							</section>
						{:else}
							<p class="text-sm text-muted-foreground">ยังไม่มีสมาชิกในหน่วยงานนี้</p>
						{/each}
					</div>
					{#if unit.children.length}{@render branch(unit.children, depth + 1)}{/if}
				</details>
			</li>
		{/each}
	</ul>
{/snippet}

{@render branch(nodes, 0)}
