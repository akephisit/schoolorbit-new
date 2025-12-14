<script lang="ts">
	import { Label } from 'bits-ui';
	import * as Button from 'bits-ui';
	import * as Checkbox from 'bits-ui';

	let email = $state('');
	let password = $state('');
	let rememberMe = $state(false);
	let isLoading = $state(false);

	async function handleSubmit(e: Event) {
		e.preventDefault();
		isLoading = true;

		// Simulate login - implement your actual login logic here
		setTimeout(() => {
			console.log('Logging in with:', { email, password, rememberMe });
			alert('Login functionality will be implemented here');
			isLoading = false;
		}, 1000);
	}

	function goBack() {
		window.location.href = '/';
	}
</script>

<svelte:head>
	<title>เข้าสู่ระบบ - SchoolOrbit</title>
</svelte:head>

<div
	class="min-h-screen bg-gradient-to-br from-indigo-500 via-purple-500 to-pink-500 flex items-center justify-center p-6"
>
	<!-- Login Card -->
	<div
		class="bg-white/95 backdrop-blur-lg rounded-3xl shadow-2xl p-8 md:p-12 w-full max-w-md transform transition hover:scale-[1.02]"
	>
		<!-- Back Button -->
		<Button.Root
			onclick={goBack}
			class="mb-6 text-indigo-600 hover:text-indigo-700 flex items-center gap-2 font-medium transition bg-transparent border-0 cursor-pointer"
		>
			← กลับหน้าหลัก
		</Button.Root>

		<!-- Logo & Title -->
		<div class="text-center mb-8">
			<div
				class="w-20 h-20 bg-gradient-to-br from-indigo-500 to-purple-500 rounded-2xl flex items-center justify-center mx-auto mb-4 shadow-lg"
			>
				<span class="text-4xl">🎓</span>
			</div>
			<h1 class="text-3xl font-bold text-gray-800 mb-2">เข้าสู่ระบบ</h1>
			<p class="text-gray-600">SchoolOrbit - ระบบบริหารจัดการโรงเรียน</p>
		</div>

		<!-- Login Form -->
		<form onsubmit={handleSubmit} class="space-y-6">
			<!-- Email Input -->
			<div>
				<Label.Root for="email" class="block text-sm font-medium text-gray-700 mb-2 cursor-pointer">
					อีเมล
				</Label.Root>
				<input
					type="email"
					id="email"
					bind:value={email}
					required
					class="w-full px-4 py-3 border-2 border-gray-200 rounded-xl focus:border-indigo-500 focus:ring-2 focus:ring-indigo-200 outline-none transition"
					placeholder="example@school.ac.th"
				/>
			</div>

			<!-- Password Input -->
			<div>
				<Label.Root
					for="password"
					class="block text-sm font-medium text-gray-700 mb-2 cursor-pointer"
				>
					รหัสผ่าน
				</Label.Root>
				<input
					type="password"
					id="password"
					bind:value={password}
					required
					class="w-full px-4 py-3 border-2 border-gray-200 rounded-xl focus:border-indigo-500 focus:ring-2 focus:ring-indigo-200 outline-none transition"
					placeholder="••••••••"
				/>
			</div>

			<!-- Remember & Forgot -->
			<div class="flex items-center justify-between text-sm">
				<Checkbox.Root bind:checked={rememberMe} class="flex items-center gap-2 cursor-pointer">
					<div class="flex items-center gap-2">
						<div
							class="w-4 h-4 rounded border-2 border-gray-300 flex items-center justify-center {rememberMe
								? 'bg-indigo-600 border-indigo-600'
								: 'bg-white'}"
						>
							{#if rememberMe}
								<svg
									class="w-3 h-3 text-white"
									fill="none"
									stroke="currentColor"
									viewBox="0 0 24 24"
								>
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="3"
										d="M5 13l4 4L19 7"
									/>
								</svg>
							{/if}
						</div>
						<Label.Root class="text-gray-600 cursor-pointer">จดจำฉันไว้</Label.Root>
					</div>
				</Checkbox.Root>

				<Button.Root
					type="button"
					class="text-indigo-600 hover:text-indigo-700 font-medium bg-transparent border-0 cursor-pointer"
				>
					ลืมรหัสผ่าน?
				</Button.Root>
			</div>

			<!-- Submit Button -->
			<Button.Root
				type="submit"
				disabled={isLoading}
				class="w-full bg-gradient-to-r from-indigo-500 to-purple-500 text-white py-3.5 rounded-xl font-semibold hover:from-indigo-600 hover:to-purple-600 transition shadow-lg hover:shadow-xl transform hover:-translate-y-0.5 disabled:opacity-50 disabled:cursor-not-allowed border-0"
			>
				{isLoading ? 'กำลังเข้าสู่ระบบ...' : 'เข้าสู่ระบบ'}
			</Button.Root>
		</form>

		<!-- Divider -->
		<div class="relative my-8">
			<div class="absolute inset-0 flex items-center">
				<div class="w-full border-t border-gray-300"></div>
			</div>
			<div class="relative flex justify-center text-sm">
				<span class="px-4 bg-white text-gray-500">หรือ</span>
			</div>
		</div>

		<!-- Register Link -->
		<div class="text-center">
			<p class="text-gray-600">
				ยังไม่มีบัญชี?
				<Button.Root
					type="button"
					class="text-indigo-600 hover:text-indigo-700 font-semibold bg-transparent border-0 cursor-pointer inline"
				>
					ลงทะเบียน
				</Button.Root>
			</p>
		</div>
	</div>
</div>
