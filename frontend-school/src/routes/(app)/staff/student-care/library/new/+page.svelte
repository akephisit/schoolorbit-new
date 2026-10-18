<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { careIdentity } from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import { careInstrument } from '#lib/api/student-care.js';
	import CareDefinitionEditor from '#lib/components/student-care/CareDefinitionEditor.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell title="เพิ่มรุ่นแบบประเมิน" backHref={careLink('/library', data.term)}
	>{#key $careIdentity + data.id}{#if data.initial && data.id}<CareRegion
				initial={data.initial}
				permissions={[PERMISSIONS.STUDENT_CARE_CONFIG_MANAGE_SCHOOL]}
				retry={(signal) => careInstrument(data.id!, { signal })}
				variant="form"
				>{#snippet children(initial)}<CareDefinitionEditor
						{initial}
						term={data.term}
					/>{/snippet}</CareRegion
			>{:else}<CareDefinitionEditor term={data.term} />{/if}{/key}</PageShell
>
