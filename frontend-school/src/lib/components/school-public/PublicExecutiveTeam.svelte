<script lang="ts">
	import type { PublicSchoolOrganization } from '#lib/api/school.js';
	import { publicExecutiveTeam } from '#lib/school-public/organization.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import PublicPersonPortrait from './PublicPersonPortrait.svelte';
	let { organization }: { organization: PublicSchoolOrganization } = $props();
	const team = $derived(publicExecutiveTeam(organization.units));
</script>

<div
	class="rounded-3xl border border-border/70 bg-card px-2 py-7 sm:px-6 sm:py-9"
	data-testid="public-executive-team"
>
	{#if team.directors.length || team.deputies.length || team.leaders.length}
		<div class="space-y-8 sm:space-y-10">
			{#each [{ label: 'ผู้อำนวยการ', people: team.directors }, { label: 'รองผู้อำนวยการ', people: team.deputies }, { label: 'ผู้รับผิดชอบกลุ่มบริหาร', people: team.leaders }] as tier (tier.label)}
				{#if tier.people.length}
					<section aria-label={tier.label}>
						<ul class="flex flex-wrap justify-center gap-y-7">
							{#each tier.people as person (person.member.personKey)}
								<li class="w-1/2 px-2 sm:w-1/3 lg:w-1/4">
									<PublicPersonPortrait {person} showUnit={tier.label !== 'ผู้อำนวยการ'} />
								</li>
							{/each}
						</ul>
					</section>
				{/if}
			{/each}
		</div>
	{:else}
		<PageState
			title="ยังไม่มีข้อมูลคณะผู้บริหาร"
			description="รายชื่อจะแสดงเมื่อโรงเรียนกำหนดผู้รับผิดชอบในโครงสร้างองค์กร"
		/>
	{/if}
</div>
