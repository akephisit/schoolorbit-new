<script lang="ts">
	import { authStore } from '$lib/stores/auth.svelte';
</script>

{#if authStore.isAuthenticated && authStore.user}
	<!-- User Info Card -->
	<div class="bg-white shadow rounded-lg p-6">
		<h2 class="text-xl font-semibold text-gray-900 mb-4">ข้อมูลผู้ใช้</h2>
		<dl class="grid grid-cols-1 gap-4 sm:grid-cols-2">
			<div>
				<dt class="text-sm font-medium text-gray-500">ชื่อ</dt>
				<dd class="mt-1 text-sm text-gray-900">{authStore.user.name}</dd>
			</div>
			<div>
				<dt class="text-sm font-medium text-gray-500">เลขบัตรประชาชน</dt>
				<dd class="mt-1 text-sm text-gray-900">{authStore.user.nationalId}</dd>
			</div>
			<div>
				<dt class="text-sm font-medium text-gray-500">บทบาท</dt>
				<dd class="mt-1 text-sm text-gray-900">
					<span class="px-2 py-1 bg-blue-100 text-blue-800 rounded-full text-xs">
						{authStore.user.role}
					</span>
				</dd>
			</div>
			<div>
				<dt class="text-sm font-medium text-gray-500">User ID</dt>
				<dd class="mt-1 text-sm text-gray-900 font-mono">{authStore.user.id}</dd>
			</div>
		</dl>
	</div>

	<!-- Quick Actions -->
	<div class="mt-6">
		<h2 class="text-xl font-semibold text-gray-900 mb-4">เมนูจัดการ</h2>
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
			<a
				href="/dashboard/schools"
				class="block p-6 bg-gradient-to-br from-purple-500 to-indigo-600 rounded-lg shadow-lg hover:shadow-xl transition-shadow"
			>
				<div class="flex items-center justify-between">
					<div>
						<h3 class="text-lg font-semibold text-white">จัดการโรงเรียน</h3>
						<p class="mt-2 text-purple-100">สร้าง แก้ไข และลบโรงเรียน</p>
					</div>
					<svg
						class="h-8 w-8 text-white opacity-80"
						fill="none"
						stroke="currentColor"
						viewBox="0 0 24 24"
					>
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width="2"
							d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"
						/>
					</svg>
				</div>
			</a>
		</div>
	</div>

	<!-- Success Message -->
	<div class="mt-6 p-4 bg-green-50 border border-green-200 rounded-md">
		<div class="flex">
			<svg class="h-5 w-5 text-green-400" fill="currentColor" viewBox="0 0 20 20">
				<path
					fill-rule="evenodd"
					d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
					clip-rule="evenodd"
				/>
			</svg>
			<p class="ml-3 text-sm text-green-700">✅ เข้าสู่ระบบสำเร็จ! Backend-admin ทำงานได้ปกติ</p>
		</div>
	</div>
{:else}
	<div class="min-h-screen flex items-center justify-center">
		<div class="text-center">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto"></div>
			<p class="mt-4 text-gray-600">กำลังโหลด...</p>
		</div>
	</div>
{/if}
