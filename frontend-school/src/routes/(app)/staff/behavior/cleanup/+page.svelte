<script lang="ts">
	import { RotateCcw } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { behaviorIdentity } from '#lib/features/behavior/access.js';
	import BehaviorCleanup from '#lib/components/behavior/BehaviorCleanup.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell
	icon={RotateCcw}
	title="รีเซ็ตและล้างข้อมูลคะแนน"
	backHref={`/staff/behavior?academicTermId=${data.term ?? ''}`}
	description="เลือกขอบเขต ตรวจผลกระทบ แล้วลบรายละเอียดหรือเปิดรอบคะแนนใหม่"
	>{#if $can.has(PERMISSIONS.BEHAVIOR_CLEANUP_EXECUTE_SCHOOL)}{#key $behaviorIdentity + data.term}<BehaviorCleanup
				term={data.term}
				year={data.year}
			/>{/key}{:else}<PageState
			variant="permission"
			title="ไม่มีสิทธิ์จัดการข้อมูลคะแนน"
		/>{/if}</PageShell
>
