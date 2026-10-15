<script lang="ts">
	import type { OrganizationHierarchyNode } from '#lib/features/staff/organization-hierarchy.js';
	import { organizationUnitLabel } from '#lib/features/staff/organization-hierarchy.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Briefcase, Building2, ChevronRight, School } from '@lucide/svelte';
	let {
		roots,
		selectedId,
		onSelect
	}: {
		roots: OrganizationHierarchyNode[];
		selectedId?: string;
		onSelect: (id: string) => void;
	} = $props();
</script>

{#snippet branch(node: OrganizationHierarchyNode)}
	<li data-organization-unit={node.unit.id} class="min-w-0">
		<Button
			variant="ghost"
			class={`h-auto w-full justify-start whitespace-normal py-2 text-left ${selectedId === node.unit.id ? 'bg-primary/10 text-primary hover:bg-primary/15' : ''} ${node.unit.unit_type === 'management_group' ? 'font-semibold' : ''}`}
			aria-current={selectedId === node.unit.id ? 'true' : undefined}
			onclick={() => onSelect(node.unit.id)}
		>
			{#if node.unit.unit_type === 'management_group'}<Briefcase class="size-4 self-start" />
			{:else if node.unit.unit_type === 'school'}<School class="size-4 self-start" />
			{:else}<Building2 class="size-4 self-start text-muted-foreground" />{/if}
			<span class="min-w-0 flex-1 break-words">
				<span class="block">{node.unit.name}</span>
				<span class="mt-0.5 block text-xs font-normal text-muted-foreground">
					{organizationUnitLabel(node.unit)}{#if !node.unit.is_active}
						· ปิดใช้งาน{/if}
				</span>
			</span>
			<ChevronRight class="size-4 text-muted-foreground" />
		</Button>
		{#if node.children.length}
			<ul
				aria-label={`ภายใต้ ${node.unit.name}`}
				class="ml-4 space-y-1 border-l border-border pl-3"
			>
				{#each node.children as child (child.unit.id)}{@render branch(child)}{/each}
			</ul>
		{/if}
	</li>
{/snippet}

<Card.Root class="gap-0 overflow-hidden py-0">
	<div class="flex items-center justify-between gap-3 border-b bg-primary/5 px-4 py-3">
		<h2 class="font-semibold">กลุ่มบริหารและฝ่าย/งาน</h2>
		<Badge variant="outline">โครงสร้างสังกัด</Badge>
	</div>
	<nav aria-label="โครงสร้างหน่วยงาน" class="p-2 lg:max-h-[40rem] lg:overflow-y-auto">
		<ul class="space-y-2">
			{#each roots as root (root.unit.id)}{@render branch(root)}{/each}
		</ul>
	</nav>
</Card.Root>
