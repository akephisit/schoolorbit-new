<script lang="ts">
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Power, Menu, Settings } from 'lucide-svelte';

	const adminFeatures = [
		{
			title: 'จัดการระบบงาน',
			description: 'เปิด/ปิดการทำงานของระบบย่อยต่างๆ',
			icon: Power,
			href: '/settings/features',
			color: 'text-blue-500'
		},
		{
			title: 'จัดการเมนู',
			description: 'จัดการโครงสร้างเมนูและการเข้าถึง',
			icon: Menu,
			href: '/settings/menu',
			color: 'text-green-500'
		}
	];
</script>

<svelte:head>
	<title>ระบบจัดการ - Admin</title>
</svelte:head>

<div class="container mx-auto p-6 space-y-6">
	<!-- Header -->
	<div>
		<h1 class="text-3xl font-bold flex items-center gap-2">
			<Settings class="h-8 w-8" />
			ระบบจัดการ
		</h1>
		<p class="text-muted-foreground mt-1">จัดการระบบงานและเมนูต่างๆ ตามสิทธิ์ที่คุณมี</p>
	</div>

	<!-- Feature Cards -->
	<div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
		{#each adminFeatures as feature}
			<a href={feature.href}>
				<Card class="p-6 hover:shadow-lg transition-shadow cursor-pointer h-full">
					<div class="space-y-4">
						<div class="flex items-center gap-3">
							<div class="p-3 bg-muted rounded-lg">
								<svelte:component this={feature.icon} class="h-6 w-6 {feature.color}" />
							</div>
							<h2 class="text-xl font-semibold">{feature.title}</h2>
						</div>
						<p class="text-muted-foreground">{feature.description}</p>
						<Button class="w-full" variant="outline">เข้าสู่ระบบ</Button>
					</div>
				</Card>
			</a>
		{/each}
	</div>

	<!-- Info Card -->
	<Card class="p-6 bg-muted/50">
		<h3 class="font-semibold mb-2">ℹ️ เกี่ยวกับสิทธิ์การเข้าถึง</h3>
		<p class="text-sm text-muted-foreground">
			คุณจะเห็นและจัดการได้เฉพาะระบบงานและเมนูที่คุณมีสิทธิ์ในโมดูลนั้นๆ ตัวอย่างเช่น หากคุณมีสิทธิ์ <code
				class="bg-background px-1 py-0.5 rounded">attendance.update.all</code
			>
			คุณสามารถจัดการระบบเช็คชื่อและเมนูที่เกี่ยวข้องได้
		</p>
	</Card>
</div>
