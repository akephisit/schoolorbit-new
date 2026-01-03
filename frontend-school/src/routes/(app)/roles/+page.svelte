<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { roleAPI, type Role } from '$lib/api/roles';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Plus, Edit, Shield } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let roles = $state<Role[]>([]);
	let loading = $state(true);

	onMount(async () => {
		await loadRoles();
	});

	async function loadRoles() {
		loading = true;
		try {
			const response = await roleAPI.listRoles();
			if (response.success && response.data) {
				roles = response.data.sort((a: Role, b: Role) => b.level - a.level);
			} else {
				toast.error('ไม่สามารถโหลดข้อมูล roles ได้');
			}
		} catch (error) {
			console.error('Failed to load roles:', error);
			toast.error('เกิดข้อผิดพลาดในการโหลดข้อมูล');
		} finally {
			loading = false;
		}
	}

	function getLevelBadgeColor(level: number): string {
		if (level >= 900) return 'bg-purple-500';
		if (level >= 80) return 'bg-red-500';
		if (level >= 50) return 'bg-orange-500';
		if (level >= 20) return 'bg-blue-500';
		return 'bg-gray-500';
	}

	function getCategoryBadgeColor(category?: string): string {
		switch (category) {
			case 'administrative':
				return 'bg-purple-100 text-purple-800';
			case 'academic':
				return 'bg-blue-100 text-blue-800';
			case 'support':
				return 'bg-green-100 text-green-800';
			default:
				return 'bg-gray-100 text-gray-800';
		}
	}
</script>

<svelte:head>
	<title>จัดการบทบาท - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold text-foreground">จัดการบทบาท</h1>
			<p class="text-muted-foreground mt-1">กำหนดบทบาทและสิทธิ์การเข้าถึงของผู้ใช้งาน</p>
		</div>
		<Button onclick={() => goto('/roles/new')} class="gap-2">
			<Plus class="h-4 w-4" />
			สร้างบทบาทใหม่
		</Button>
	</div>

	{#if loading}
		<!-- Loading State -->
		<div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
			{#each Array(6) as _}
				<Card>
					<CardHeader>
						<div class="h-6 bg-gray-200 rounded animate-pulse w-3/4"></div>
						<div class="h-4 bg-gray-200 rounded animate-pulse w-1/2 mt-2"></div>
					</CardHeader>
					<CardContent>
						<div class="h-4 bg-gray-200 rounded animate-pulse"></div>
					</CardContent>
				</Card>
			{/each}
		</div>
	{:else if roles.length === 0}
		<!-- Empty State -->
		<Card>
			<CardContent class="py-12">
				<div class="text-center">
					<Shield class="h-12 w-12 text-muted-foreground mx-auto mb-4" />
					<h3 class="text-lg font-medium text-foreground">ยังไม่มีบทบาท</h3>
					<p class="text-muted-foreground mt-1">เริ่มต้นสร้างบทบาทแรกของคุณ</p>
					<Button onclick={() => goto('/roles/new')} class="mt-4 gap-2">
						<Plus class="h-4 w-4" />
						สร้างบทบาทใหม่
					</Button>
				</div>
			</CardContent>
		</Card>
	{:else}
		<!-- Roles Grid -->
		<div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
			{#each roles as role (role.id)}
				<Card
					class="hover:shadow-lg transition-shadow cursor-pointer"
					onclick={() => goto(`/roles/${role.id}`)}
				>
					<CardHeader>
						<div class="flex items-start justify-between">
							<div class="flex-1 min-w-0">
								<CardTitle class="truncate">{role.name}</CardTitle>
								<CardDescription class="truncate">{role.code}</CardDescription>
							</div>
							<Badge class="{getLevelBadgeColor(role.level)} text-white">
								Lv {role.level}
							</Badge>
						</div>
					</CardHeader>
					<CardContent class="space-y-3">
						<!-- Description -->
						{#if role.description}
							<p class="text-sm text-muted-foreground line-clamp-2">
								{role.description}
							</p>
						{/if}

						<!-- Stats -->
						<div class="flex items-center gap-4 text-sm">
							<div class="flex items-center gap-1 text-muted-foreground">
								<Shield class="h-4 w-4" />
								<span>
									{role.permissions.includes('*') ? 'All' : role.permissions.length} สิทธิ์
								</span>
							</div>
							{#if role.category}
								<Badge class={getCategoryBadgeColor(role.category)} variant="secondary">
									{role.category}
								</Badge>
							{/if}
						</div>

						<!-- Status -->
						<div class="flex items-center justify-between pt-2 border-t">
							<Badge variant={role.is_active ? 'default' : 'secondary'}>
								{role.is_active ? 'ใช้งาน' : 'ไม่ใช้งาน'}
							</Badge>
							<Button
								variant="ghost"
								size="sm"
								onclick={(e: MouseEvent) => {
									e.stopPropagation();
									goto(`/roles/${role.id}`);
								}}
								class="gap-1"
							>
								<Edit class="h-3 w-3" />
								แก้ไข
							</Button>
						</div>
					</CardContent>
				</Card>
			{/each}
		</div>
	{/if}
</div>

<style>
	:global(body) {
		font-family: 'Kanit', sans-serif;
	}
</style>
