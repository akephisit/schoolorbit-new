<script lang="ts">
	import { resolve } from '$app/paths';
	import AcademicSetupRegion from '#lib/components/academic-core/AcademicSetupRegion.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<PageShell title="เวลาเรียนและคาบเรียน" description="จัดตารางเวลาและคาบเรียนที่ใช้กับภาคเรียน">
	<Button variant="outline" href={resolve('staff/academic/year-lifecycle')}>จัดการปีการศึกษา</Button
	>
	<AcademicSetupRegion workspaceLoad={data.workspace} mode="time" />
</PageShell>
