<script lang="ts">
	import { authStore } from '$lib/stores/auth.svelte';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	let nationalId = $state('');
	let password = $state('');
	let validationError = $state('');

	// Redirect if already authenticated
	onMount(() => {
		if (authStore.isAuthenticated) {
			goto('/dashboard');
		}
	});

	function validateNationalId(id: string): boolean {
		// Thai National ID: exactly 13 digits
		return /^\d{13}$/.test(id);
	}

	function handleInput() {
		validationError = '';
		authStore.clearError();
	}

	async function handleSubmit(e: Event) {
		e.preventDefault();

		// Validation
		if (!nationalId || !password) {
			validationError = 'กรุณากรอกข้อมูลให้ครบถ้วน';
			return;
		}

		if (!validateNationalId(nationalId)) {
			validationError = 'เลขบัตรประชาชนต้องเป็นตัวเลข 13 หลัก';
			return;
		}

		// Clear validation error
		validationError = '';

		// Attempt login
		await authStore.login({ nationalId, password });
	}
</script>

<div class="min-h-screen flex items-center justify-center bg-gray-50 px-4">
	<div class="max-w-md w-full space-y-8">
		<!-- Header -->
		<div class="text-center">
			<h1 class="text-3xl font-bold text-gray-900">SchoolOrbit Admin</h1>
			<p class="mt-2 text-sm text-gray-600">เข้าสู่ระบบด้วยเลขบัตรประชาชน</p>
		</div>

		<!-- Login Form -->
		<form onsubmit={handleSubmit} class="mt-8 space-y-6 bg-white p-8 rounded-lg shadow-md">
			<!-- National ID Input -->
			<div>
				<label for="nationalId" class="block text-sm font-medium text-gray-700">
					เลขบัตรประชาชน
				</label>
				<input
					id="nationalId"
					type="text"
					inputmode="numeric"
					bind:value={nationalId}
					oninput={handleInput}
					maxlength="13"
					placeholder="1234567890123"
					class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
					required
					disabled={authStore.isLoading}
				/>
			</div>

			<!-- Password Input -->
			<div>
				<label for="password" class="block text-sm font-medium text-gray-700">
					รหัสผ่าน
				</label>
				<input
					id="password"
					type="password"
					bind:value={password}
					oninput={handleInput}
					placeholder="••••••••"
					class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
					required
					disabled={authStore.isLoading}
				/>
			</div>

			<!-- Error Messages -->
			{#if validationError}
				<div class="p-3 bg-red-50 border border-red-200 rounded-md">
					<p class="text-sm text-red-600">{validationError}</p>
				</div>
			{/if}

			{#if authStore.error}
				<div class="p-3 bg-red-50 border border-red-200 rounded-md">
					<p class="text-sm text-red-600">{authStore.error}</p>
				</div>
			{/if}

			<!-- Submit Button -->
			<button
				type="submit"
				disabled={authStore.isLoading}
				class="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
			>
				{#if authStore.isLoading}
					<span class="flex items-center">
						<svg
							class="animate-spin -ml-1 mr-3 h-5 w-5 text-white"
							xmlns="http://www.w3.org/2000/svg"
							fill="none"
							viewBox="0 0 24 24"
						>
							<circle
								class="opacity-25"
								cx="12"
								cy="12"
								r="10"
								stroke="currentColor"
								stroke-width="4"
							></circle>
							<path
								class="opacity-75"
								fill="currentColor"
								d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
							></path>
						</svg>
						กำลังเข้าสู่ระบบ...
					</span>
				{:else}
					เข้าสู่ระบบ
				{/if}
			</button>

			<!-- Test Credentials Info -->
			<div class="mt-4 p-3 bg-blue-50 border border-blue-200 rounded-md">
				<p class="text-xs text-blue-700 font-medium">ข้อมูลทดสอบ:</p>
				<p class="text-xs text-blue-600 mt-1">เลขบัตรประชาชน: 1234567890123</p>
				<p class="text-xs text-blue-600">รหัสผ่าน: test123</p>
			</div>
		</form>
	</div>
</div>
