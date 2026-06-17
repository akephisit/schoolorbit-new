<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '$lib/components/app-layout';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Tabs, TabsContent, TabsList, TabsTrigger } from '$lib/components/ui/tabs';
	import { User, Shield } from 'lucide-svelte';
	import UserRoleManager from '$lib/components/UserRoleManager.svelte';

	let { params }: PageProps = $props();
	let userId = $derived(params.id);
	let activeTab = $state('roles');
</script>

<svelte:head>
	<title>จัดการสิทธิ์ - SchoolOrbit</title>
</svelte:head>

<PageShell title="จัดการสิทธิ์ผู้ใช้งาน" description="บทบาทและสิทธิ์การเข้าถึง" backHref="/staff">
	<Tabs bind:value={activeTab}>
		<TabsList>
			<TabsTrigger value="roles" class="gap-2">
				<Shield class="h-4 w-4" />
				บทบาทและสิทธิ์
			</TabsTrigger>
			<TabsTrigger value="profile" class="gap-2">
				<User class="h-4 w-4" />
				ข้อมูลส่วนตัว
			</TabsTrigger>
		</TabsList>

		<TabsContent value="roles" class="mt-6">
			<UserRoleManager {userId} />
		</TabsContent>

		<TabsContent value="profile" class="mt-6">
			<Card>
				<CardHeader>
					<CardTitle>ข้อมูลส่วนตัว</CardTitle>
				</CardHeader>
				<CardContent>
					<p class="text-muted-foreground">ข้อมูลโปรไฟล์ผู้ใช้งาน...</p>
				</CardContent>
			</Card>
		</TabsContent>
	</Tabs>
</PageShell>

<style>
	:global(body) {
		font-family: 'Kanit', sans-serif;
	}
</style>
