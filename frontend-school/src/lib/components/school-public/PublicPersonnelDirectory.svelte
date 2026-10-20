<script lang="ts">
	import type { PublicSchoolOrganization } from '#lib/api/school.js';
	import {
		publicPersonnelGroups,
		type PublicPersonnelGroup
	} from '#lib/school-public/organization.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import PublicPersonPortrait from './PublicPersonPortrait.svelte';
	let {
		organization,
		unitType
	}: { organization: PublicSchoolOrganization; unitType: 'management_group' | 'subject_group' } =
		$props();
	const groups = $derived(publicPersonnelGroups(organization.units, unitType));
	const label = $derived(unitType === 'management_group' ? 'กลุ่มบริหาร' : 'กลุ่มสาระ');
	const directoryPath = $derived(
		unitType === 'management_group' ? '/management-groups' : '/subject-groups'
	);
</script>

{#snippet unitContent(group: PublicPersonnelGroup, depth: number)}
	<section
		aria-labelledby={`unit-title-${group.unit.id}`}
		class={depth === 0
			? 'scroll-mt-24 rounded-3xl border border-border/70 bg-card px-3 py-6 sm:px-6 sm:py-8'
			: 'mt-7 border-t border-border/70 pt-6'}
		id={`unit-${group.unit.id}`}
	>
		{#if depth === 0}<h2
				id={`unit-title-${group.unit.id}`}
				class="text-center text-lg font-medium sm:text-xl"
			>
				{group.unit.name}
			</h2>
		{:else}<h3 id={`unit-title-${group.unit.id}`} class="text-center text-base font-medium">
				{group.unit.name}
			</h3>{/if}
		{#if group.leaders.length}
			<ul
				aria-label={`ผู้รับผิดชอบ ${group.unit.name}`}
				class="mt-7 flex flex-wrap justify-center gap-y-7"
			>
				{#each group.leaders as person (person.member.personKey)}<li
						class="w-1/2 px-2 sm:w-1/3 lg:w-1/4"
					>
						<PublicPersonPortrait {person} showUnit />
					</li>{/each}
			</ul>
		{/if}
		{#if group.members.length}
			<ul
				aria-label={`สมาชิก ${group.unit.name}`}
				class="mt-7 flex flex-wrap justify-center gap-y-7"
			>
				{#each group.members as person (person.member.personKey)}<li
						class="w-1/2 min-w-0 px-2 sm:w-1/3 lg:w-1/4"
					>
						<PublicPersonPortrait {person} showUnit />
					</li>{/each}
			</ul>
		{:else if !group.leaders.length}
			<p class="mt-4 text-center text-sm text-muted-foreground">
				ยังไม่มีรายชื่อบุคลากรในหน่วยงานนี้
			</p>
		{/if}
		{#each group.children as child (child.unit.id)}{@render unitContent(child, depth + 1)}{/each}
	</section>
{/snippet}

<div class="space-y-5" data-testid="public-personnel-directory">
	{#if groups.length}
		<nav aria-label={`เลือก${label}`} class="flex flex-wrap justify-center gap-2 pb-1">
			{#each groups as group (group.unit.id)}<Button
					href={`${directoryPath}#unit-${group.unit.id}`}
					variant="outline"
					size="sm"
					class="h-auto min-h-10 max-w-full rounded-full border-border/70 bg-card px-4 py-2 text-center whitespace-normal shadow-none"
					>{group.unit.name}</Button
				>{/each}
		</nav>
		{#each groups as group (group.unit.id)}{@render unitContent(group, 0)}{/each}
	{:else}
		<PageState
			title={`ยังไม่มีข้อมูล${label}`}
			description="รายชื่อจะแสดงเมื่อโรงเรียนตั้งค่ากลุ่มและสมาชิก"
		/>
	{/if}
</div>
