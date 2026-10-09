<script lang="ts">
	import { onMount } from 'svelte';
	import type { PublicOrganizationNode } from '#lib/school-public/organization.js';
	import { groupPublicOrganizationMembers } from '#lib/school-public/organization.js';
	import { BACKEND_URL } from '#lib/api/client.js';
	import { Avatar } from '#lib/components/ui/avatar/index.js';
	import { Building2, ChevronDown } from '@lucide/svelte';
	let { nodes }: { nodes: PublicOrganizationNode[] } = $props();
	let chart: HTMLDivElement;
	function centerChart() {
		if (chart) chart.scrollLeft = (chart.scrollWidth - chart.clientWidth) / 2;
	}
	onMount(centerChart);
</script>

{#snippet branch(items: PublicOrganizationNode[], depth: number)}
	<ul class="flex w-max min-w-full justify-center">
		{#each items as unit (unit.id)}
			<li class="relative flex flex-col items-center px-3" class:org-node={depth > 0}>
				<details
					open={depth < 2}
					ontoggle={depth === 0 ? centerChart : undefined}
					class="flex flex-col items-center"
				>
					<summary
						class="public-surface flex w-64 cursor-pointer list-none items-start gap-2 rounded-xl border bg-card p-3"
					>
						<Building2 class="mt-0.5 size-4 shrink-0 text-primary" />
						<div class="min-w-0 flex-1">
							<h3 class="text-sm font-semibold">{unit.name}</h3>
							<p class="mt-1 text-xs text-muted-foreground">
								{unit.members.length} คน{#if unit.children.length}
									· {unit.children.length} หน่วยงานย่อย{/if}
							</p>
						</div>
						<ChevronDown
							class="mt-0.5 size-4 shrink-0 text-muted-foreground transition-transform unit-chevron"
						/>
					</summary>
					<div class="flex flex-col items-center">
						{#if unit.members.length}
							<div
								class="mt-2 max-h-80 w-64 space-y-3 overflow-y-auto rounded-xl border bg-card p-3"
							>
								{#each groupPublicOrganizationMembers(unit.members) as group (group.code)}
									<section aria-label={group.label}>
										<h4
											class="mb-2 flex items-center justify-between gap-2 text-xs font-medium text-primary"
										>
											<span>{group.label}</span><span class="font-normal text-muted-foreground"
												>{group.members.length} คน</span
											>
										</h4>
										<ul class="space-y-2">
											{#each group.members as member, index (index)}
												<li class="flex items-center gap-2">
													<Avatar
														src={member.avatarUrl ? `${BACKEND_URL}${member.avatarUrl}` : null}
														alt={`รูป ${member.name}`}
														size="sm"
														class="shrink-0"
													/>
													<div class="min-w-0">
														<p class="break-words text-xs font-medium">{member.name}</p>
														{#if member.positionTitle}<p
																class="mt-0.5 text-xs text-muted-foreground"
															>
																{member.positionTitle}
															</p>{/if}
													</div>
												</li>
											{/each}
										</ul>
									</section>
								{/each}
							</div>
						{:else if !unit.children.length}
							<p class="mt-2 w-64 text-center text-xs text-muted-foreground">
								ยังไม่มีสมาชิกในหน่วยงานนี้
							</p>
						{/if}
						{#if unit.children.length}
							<div class="h-6 border-l border-border" aria-hidden="true"></div>
							{@render branch(unit.children, depth + 1)}
						{/if}
					</div>
				</details>
			</li>
		{/each}
	</ul>
{/snippet}

<p class="mb-3 text-xs text-muted-foreground">
	กดหน่วยงานเพื่อกางผัง · เลื่อนซ้าย–ขวาเพื่อดูหน่วยงานทั้งหมด
</p>
<div
	bind:this={chart}
	class="overflow-x-auto rounded-xl border bg-muted/20 p-4"
	role="region"
	aria-label="แผนผังบุคลากรและหน่วยงาน"
	data-testid="public-organization-chart"
>
	{@render branch(nodes, 0)}
</div>

<style>
	:global(details[open] > summary .unit-chevron) {
		transform: rotate(180deg);
	}
	.org-node {
		padding-top: 1.5rem;
	}
	.org-node::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		border-top: 1px solid var(--border);
	}
	.org-node::after {
		content: '';
		position: absolute;
		top: 0;
		left: 50%;
		height: 1.5rem;
		border-left: 1px solid var(--border);
	}
	.org-node:first-child::before {
		left: 50%;
	}
	.org-node:last-child::before {
		right: 50%;
	}
	.org-node:only-child::before {
		border-top: 0;
	}
</style>
