<script lang="ts">
	import { BACKEND_URL } from '#lib/api/client.js';
	import { Avatar } from '#lib/components/ui/avatar/index.js';
	import type { PublicPerson } from '#lib/school-public/organization.js';
	let { person, showUnit = false }: { person: PublicPerson; showUnit?: boolean } = $props();
</script>

<div
	class="mx-auto flex w-full max-w-52 min-w-0 flex-col items-center text-center"
	data-testid="public-person"
>
	<Avatar
		src={person.member.avatarUrl ? `${BACKEND_URL}${person.member.avatarUrl}` : null}
		alt={`รูป ${person.member.name}`}
		size="xl"
		class="mb-3 aspect-square h-auto w-32 max-w-full shrink-0 ring-4 ring-primary/5 sm:w-36 lg:w-40"
	/>
	<p class="w-full text-sm leading-relaxed font-medium break-words sm:text-base">
		{person.member.name}
	</p>
	{#each person.responsibilities as responsibility (`${responsibility.unitId}:${responsibility.title}`)}
		<p class="mt-1 w-full text-xs leading-relaxed break-words text-muted-foreground sm:text-sm">
			{responsibility.title}{#if showUnit}<span class="block">{responsibility.unitName}</span>{/if}
		</p>
	{/each}
</div>
