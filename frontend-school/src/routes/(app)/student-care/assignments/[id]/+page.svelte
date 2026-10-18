<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import {
		careIdentity,
		careReadPermissions,
		careSensitiveReadPermissions
	} from '#lib/features/student-care/access.js';

	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import { authStore } from '#lib/stores/auth.js';
	import { careAssignment, careRound } from '#lib/api/student-care.js';
	import CareResponse from '#lib/components/student-care/CareResponse.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell
	title="แบบประเมินเพื่อการดูแล"
	backHref={`/${$authStore.user?.user_type ?? 'staff'}/student-care`}
	>{#key $careIdentity + data.id}<CareRegion
			initial={data.initial}
			permissions={[...careReadPermissions, ...careSensitiveReadPermissions]}
			retry={async (signal) => {
				const detail = await careAssignment(data.id, { signal });
				const round = await careRound(detail.assignment.roundId, { signal });
				return { detail, round };
			}}
			variant="form"
			>{#snippet children(initial)}<CareResponse
					initial={initial.detail}
					round={initial.round}
				/>{/snippet}</CareRegion
		>{/key}</PageShell
>
