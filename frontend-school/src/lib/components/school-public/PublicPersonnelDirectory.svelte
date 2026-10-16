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
			? 'scroll-mt-24 border-b border-border py-8 last:border-0 sm:py-10'
			: 'mt-8 border-t border-border pt-7'}
		id={`unit-${group.unit.id}`}
	>
		{#if depth === 0}<h2 id={`unit-title-${group.unit.id}`} class="text-lg font-medium sm:text-xl">
				{group.unit.name}
			</h2>
		{:else}<h3 id={`unit-title-${group.unit.id}`} class="text-base font-medium">
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
						<PublicPersonPortrait {person} />
					</li>{/each}
			</ul>
		{/if}
		{#if group.members.length}
			<ul
				aria-label={`สมาชิก ${group.unit.name}`}
				class="mt-7 grid grid-cols-2 gap-x-4 gap-y-7 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5"
			>
				{#each group.members as person (person.member.personKey)}<li class="min-w-0">
						<PublicPersonPortrait {person} />
					</li>{/each}
			</ul>
		{:else if !group.leaders.length}
			<p class="mt-4 text-sm text-muted-foreground">ยังไม่มีรายชื่อบุคลากรในหน่วยงานนี้</p>
		{/if}
		{#each group.children as child (child.unit.id)}{@render unitContent(child, depth + 1)}{/each}
	</section>
{/snippet}

<div data-testid="public-personnel-directory">
	{#if groups.length}
		<nav aria-label={`เลือก${label}`} class="flex flex-wrap gap-2 border-b border-border pb-5">
			{#each groups as group (group.unit.id)}<Button
					href={`${directoryPath}#unit-${group.unit.id}`}
					variant="outline"
					size="sm"
					class="h-auto min-h-8 max-w-full py-1.5 text-left whitespace-normal"
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
