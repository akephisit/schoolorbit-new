<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Tabs, TabsContent, TabsList, TabsTrigger } from '$lib/components/ui/tabs';
	import { ArrowLeft, User, Shield } from 'lucide-svelte';
	import UserRoleManager from '$lib/components/UserRoleManager.svelte';

	let userId = $page.params.id || '';
	let activeTab = 'roles';


	// This would normally come from API
	let userData = {
		id: userId,
		firstName: 'Loading...',
		lastName: '',
		userType: 'staff'
	};
</script>

<svelte:head>
	<title>จัดการสิทธิ์ - SchoolOrbit</title>
</svelte:head>

<div class="container mx-auto py-6 px-4 max-w-6xl">
	<!-- Header -->
	<div class="flex items-center gap-4 mb-6">
		<Button variant="ghost" size="icon" on:click={() => goto('/staff')}>
			<ArrowLeft class="h-5 w-5" />
		</Button>
		<div class="flex-1">
			<h1 class="text-3xl font-bold text-gray-900">จัดการสิทธิ์ผู้ใช้งาน</h1>
			<p class="text-gray-600 mt-1">บทบาทและสิทธิ์การเข้าถึง</p>
		</div>
	</div>

	<!-- Tabs -->
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
					<p class="text-gray-600">ข้อมูลโปรไฟล์ผู้ใช้งาน...</p>
				</CardContent>
			</Card>
		</TabsContent>
	</Tabs>
</div>

<style>
	:global(body) {
		font-family: 'Kanit', sans-serif;
	}
</style>
