<script lang="ts">
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	import SelfRecordedAchievements from '$lib/components/achievement/SelfRecordedAchievements.svelte';
</script>

<SelfRecordedAchievements initialAchievements={data.achievements} />
