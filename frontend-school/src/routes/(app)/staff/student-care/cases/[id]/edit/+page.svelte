<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import {
		careIdentity,
		careSensitiveManagePermissions
	} from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import { careCase, careStaff } from '#lib/api/student-care.js';
	import CareCaseForm from '#lib/components/student-care/CareCaseForm.svelte';
	let { data }: PageProps = $props();
	async function retry(signal: AbortSignal) {
		const [record, staff] = await Promise.all([
			careCase(data.id, { signal }),
			careStaff({ signal })
		]);
		return { record, staff };
	}
</script>

<PageShell title="แก้ผู้รับผิดชอบและแผนการดูแล" backHref={careLink(`/cases/${data.id}`, data.term)}
	>{#key $careIdentity + data.id}<CareRegion
			initial={data.initial}
			permissions={careSensitiveManagePermissions}
			{retry}
			variant="form"
			>{#snippet children(options)}<CareCaseForm
					initial={options.record}
					staff={options.staff}
					students={[]}
					term={data.term}
					date={data.date}
				/>{/snippet}</CareRegion
		>{/key}</PageShell
>
