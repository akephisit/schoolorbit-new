<script lang="ts">
	import type { PageProps } from './$types';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { ArrowLeft, Home, ShieldAlert } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import { dashboardPathForUser } from '$lib/auth/route-access';
	import { authStore } from '$lib/stores/auth';

	const { data }: PageProps = $props();
	let dashboardPath = $derived(dashboardPathForUser($authStore.user));

	function goBack() {
		if (window.history.length > 1) {
			window.history.back();
			return;
		}

		void goto(resolve(dashboardPath));
	}
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<section class="min-h-full flex items-center justify-center px-4 py-12">
	<div class="w-full max-w-xl space-y-6 text-center">
		<div
			class="mx-auto flex h-16 w-16 items-center justify-center rounded-full border border-destructive/20 bg-destructive/10 text-destructive"
		>
			<ShieldAlert class="h-8 w-8" />
		</div>

		<div class="space-y-2">
			<p class="text-sm font-medium text-destructive">403</p>
			<h1 class="text-2xl font-bold tracking-tight sm:text-3xl">ไม่มีสิทธิ์เข้าถึงหน้านี้</h1>
			<p class="text-muted-foreground">
				บัญชีของคุณยังไม่มีสิทธิ์สำหรับหน้านี้ หากคิดว่าเป็นความผิดพลาด กรุณาติดต่อผู้ดูแลระบบ
			</p>
			{#if data.from}
				<p class="truncate text-xs text-muted-foreground/80">เส้นทางที่ร้องขอ: {data.from}</p>
			{/if}
		</div>

		<div class="flex flex-col justify-center gap-3 sm:flex-row">
			<Button variant="outline" onclick={goBack}>
				<ArrowLeft class="h-4 w-4" />
				ย้อนกลับ
			</Button>
			<Button href={dashboardPath}>
				<Home class="h-4 w-4" />
				กลับแดชบอร์ด
			</Button>
		</div>
	</div>
</section>
