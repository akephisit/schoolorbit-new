<script lang="ts">
	import { HeartHandshake } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import { careIdentity, careSensitiveReadPermissions } from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import { careCase } from '#lib/api/student-care.js';
	import CareCaseDetail from '#lib/components/student-care/CareCaseDetail.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell
	backLabel="กลับรายการดูแลรายกรณี"
	icon={HeartHandshake}
	title="รายละเอียดการดูแล"
	backHref={careLink('/cases', data.term)}
	>{#key $careIdentity + data.id}<CareRegion
			initial={data.initial}
			permissions={careSensitiveReadPermissions}
			retry={(signal) => careCase(data.id, { signal })}
			variant="detail"
			>{#snippet children(initial)}<CareCaseDetail
					{initial}
					term={data.term}
				/>{/snippet}</CareRegion
		>{/key}</PageShell
>
