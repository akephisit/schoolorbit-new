<script lang="ts">
	import { onMount } from 'svelte';
	import { listRoles, type Role } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Shield, Plus, Pencil, Search } from 'lucide-svelte';

	let roles: Role[] = $state([]);
	let loading = $state(true);
	let error = $state('');
	let searchQuery = $state('');

	let filteredRoles = $derived(
		roles.filter(
			(role) =>
				role.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
				role.code.toLowerCase().includes(searchQuery.toLowerCase()) ||
				(role.name_en && role.name_en.toLowerCase().includes(searchQuery.toLowerCase()))
		)
	);

	async function loadRoles() {
		try {
			loading = true;
			error = '';
			const response = await listRoles();

			if (response.success && response.data) {
				roles = response.data;
			} else {
				error = response.error || 'ไม่สามารถโหลดข้อมูลบทบาทได้';
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'เกิดข้อผิดพลาด';
			console.error('Failed to load roles:', e);
		} finally {
			loading = false;
		}
	}

	function getCategoryLabel(category: string): string {
		const labels: Record<string, string> = {
			administrative: 'บริหาร',
			teaching: 'สอน',
			operational: 'ปฏิบัติการ',
			support: 'สนับสนุน'
		};
		return labels[category] || category;
	}

	function getCategoryColor(category: string): string {
		const colors: Record<string, string> = {
			administrative: 'bg-purple-100 text-purple-800',
			teaching: 'bg-blue-100 text-blue-800',
			operational: 'bg-green-100 text-green-800',
			support: 'bg-gray-100 text-gray-800'
		};
		return colors[category] || 'bg-gray-100 text-gray-800';
	}

	onMount(() => {
		loadRoles();
	});
</script>

<svelte:head>
	<title>จัดการบทบาท - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
		<div>
			<h1 class="text-3xl font-bold text-foreground flex items-center gap-2">
				<Shield class="w-8 h-8" />
				จัดการบทบาท
			</h1>
			<p class="text-muted-foreground mt-1">จัดการบทบาทและสิทธิ์การเข้าถึง</p>
		</div>
		<Button disabled class="flex items-center gap-2 opacity-50 cursor-not-allowed">
			<Plus class="w-4 h-4" />
			เพิ่มบทบาท (ติดต่อผู้ดูแลระบบ)
		</Button>
	</div>

	<!-- Search Bar -->
	<div class="bg-card border border-border rounded-lg p-4">
		<div class="relative">
			<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
			<Input type="text" bind:value={searchQuery} placeholder="ค้นหาบทบาท..." class="pl-10" />
		</div>
	</div>

	<!-- Roles List -->
	{#if loading}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<div
				class="inline-block w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin"
			></div>
			<p class="mt-4 text-muted-foreground">กำลังโหลด...</p>
		</div>
	{:else if error}
		<div class="bg-destructive/10 border border-destructive/20 rounded-lg p-6 text-center">
			<p class="text-destructive">{error}</p>
			<Button onclick={loadRoles} variant="outline" class="mt-4">ลองอีกครั้ง</Button>
		</div>
	{:else if filteredRoles.length === 0}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<Shield class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
			<p class="text-lg font-medium text-foreground">ไม่พบบทบาท</p>
			<p class="text-muted-foreground mt-2">ลองค้นหาด้วยคำอื่น</p>
		</div>
	{:else}
		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
			{#each filteredRoles as role}
				<div class="bg-card border border-border rounded-lg p-6 hover:shadow-md transition-shadow">
					<div class="flex items-start justify-between mb-4">
						<div class="flex-1">
							<h3 class="text-lg font-semibold text-foreground">{role.name}</h3>
							{#if role.name_en}
								<p class="text-sm text-muted-foreground">{role.name_en}</p>
							{/if}
						</div>
						<Button disabled size="sm" variant="ghost" class="opacity-50 cursor-not-allowed">
							<Pencil class="w-4 h-4" />
						</Button>
					</div>

					<div class="space-y-3">
						<div class="flex items-center gap-2">
							<span
								class="text-xs px-2.5 py-1 rounded-full font-medium {getCategoryColor(
									role.category
								)}"
							>
								{getCategoryLabel(role.category)}
							</span>
							<span
								class="text-xs px-2.5 py-1 bg-muted text-muted-foreground rounded-full font-medium"
							>
								รหัส: {role.code}
							</span>
						</div>

						<div class="text-sm">
							<span class="text-muted-foreground">ระดับอำนาจ:</span>
							<span class="font-medium ml-2">{role.level}</span>
						</div>

						{#if role.description}
							<p class="text-sm text-muted-foreground line-clamp-2">{role.description}</p>
						{/if}

						<div class="pt-3 border-t border-border">
							<p class="text-xs text-muted-foreground mb-2">สิทธิ์ที่มี:</p>
							<div class="flex flex-wrap gap-1">
								{#if Array.isArray(role.permissions) && role.permissions.length > 0}
									{#each role.permissions.slice(0, 3) as permission}
										<span class="text-xs px-2 py-0.5 bg-primary/10 text-primary rounded">
											{permission}
										</span>
									{/each}
									{#if role.permissions.length > 3}
										<span class="text-xs px-2 py-0.5 bg-muted text-muted-foreground rounded">
											+{role.permissions.length - 3}
										</span>
									{/if}
								{:else}
									<span class="text-xs text-muted-foreground">ไม่มีสิทธิ์</span>
								{/if}
							</div>
						</div>

						<div class="flex items-center justify-between pt-2">
							<span class="text-xs {role.is_active ? 'text-green-600' : 'text-gray-500'}">
								{role.is_active ? '● ใช้งาน' : '○ ไม่ใช้งาน'}
							</span>
							<span class="text-xs text-muted-foreground">
								สร้าง: {new Date(role.created_at).toLocaleDateString('th-TH')}
							</span>
						</div>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
