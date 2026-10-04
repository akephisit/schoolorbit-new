<script lang="ts">
	import type { PageProps } from './$types';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { ArrowLeft, Home } from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { dashboardPathForUser } from '#lib/auth/route-access.js';
	import { authStore } from '#lib/stores/auth.js';

	const { data }: PageProps = $props();
	let dashboardPath = $derived(dashboardPathForUser($authStore.user));

	function goBack() {
		if (window.history.length > 1) {
			window.history.back();
			return;
		}

		void goto(
			resolve(
				dashboardPath === '/staff' ? 'staff' : dashboardPath === '/student' ? 'student' : 'parent'
			)
		);
	}
</script>

<PageShell title="สิทธิ์การเข้าถึง" description="ตรวจสอบสิทธิ์ของบัญชีก่อนเข้าใช้งานหน้านี้">
	<PageState
		variant="permission"
		title="ไม่มีสิทธิ์เข้าถึงหน้านี้"
		description="บัญชีของคุณยังไม่มีสิทธิ์สำหรับหน้านี้ หากคิดว่าเป็นความผิดพลาด กรุณาติดต่อผู้ดูแลระบบ"
	/>

	{#if data.from}
		<p class="truncate text-xs text-muted-foreground/80">เส้นทางที่ร้องขอ: {data.from}</p>
	{/if}

	<div class="flex flex-col gap-3 sm:flex-row">
		<Button variant="outline" onclick={goBack}>
			<ArrowLeft class="h-4 w-4" />
			ย้อนกลับ
		</Button>
		<Button href={dashboardPath}>
			<Home class="h-4 w-4" />
			กลับแดชบอร์ด
		</Button>
	</div>
</PageShell>
