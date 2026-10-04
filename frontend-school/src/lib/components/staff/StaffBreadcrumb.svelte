<script lang="ts">
	import { resolve } from '$app/paths';
	let {
		returnHref,
		name,
		profileHref,
		current
	}: {
		returnHref: string;
		name?: string;
		profileHref?: string;
		current?: string;
	} = $props();
</script>

<nav
	aria-label="เส้นทางหน้าบุคลากร"
	class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
>
	<a
		href={resolve(returnHref.slice(1) as `staff/manage${'' | `?${string}`}`)}
		data-sveltekit-preload-data={false}
		class="hover:text-foreground">รายชื่อบุคลากร</a
	>
	{#if name}
		<span aria-hidden="true">/</span>
		{#if profileHref}<a
				href={resolve(profileHref.slice(1) as `staff/manage/${string}`)}
				data-sveltekit-preload-data="tap"
				class="hover:text-foreground">{name}</a
			>
		{:else}<span aria-current={current ? undefined : 'page'} class="text-foreground">{name}</span
			>{/if}
	{/if}
	{#if current}<span aria-hidden="true">/</span><span aria-current="page" class="text-foreground"
			>{current}</span
		>{/if}
</nav>
