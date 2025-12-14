<script lang="ts">
	let email = $state('');
	let password = $state('');
	let isLoading = $state(false);

	async function handleSubmit(e: Event) {
		e.preventDefault();
		isLoading = true;

		// Simulate login - implement your actual login logic here
		setTimeout(() => {
			console.log('Logging in with:', { email, password });
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
		<button
			onclick={goBack}
			class="mb-6 text-indigo-600 hover:text-indigo-700 flex items-center gap-2 font-medium transition"
		>
			← กลับหน้าหลัก
		</button>

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
				<label for="email" class="block text-sm font-medium text-gray-700 mb-2"> อีเมล </label>
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
				<label for="password" class="block text-sm font-medium text-gray-700 mb-2">
					รหัสผ่าน
				</label>
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
				<label class="flex items-center gap-2 cursor-pointer">
					<input
						type="checkbox"
						class="w-4 h-4 rounded border-gray-300 text-indigo-600 focus:ring-indigo-500"
					/>
					<span class="text-gray-600">จดจำฉันไว้</span>
				</label>
				<button type="button" class="text-indigo-600 hover:text-indigo-700 font-medium">
					ลืมรหัสผ่าน?
				</button>
			</div>

			<!-- Submit Button -->
			<button
				type="submit"
				disabled={isLoading}
				class="w-full bg-gradient-to-r from-indigo-500 to-purple-500 text-white py-3.5 rounded-xl font-semibold hover:from-indigo-600 hover:to-purple-600 transition shadow-lg hover:shadow-xl transform hover:-translate-y-0.5 disabled:opacity-50 disabled:cursor-not-allowed"
			>
				{isLoading ? 'กำลังเข้าสู่ระบบ...' : 'เข้าสู่ระบบ'}
			</button>
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
				<button type="button" class="text-indigo-600 hover:text-indigo-700 font-semibold">
					ลงทะเบียน
				</button>
			</p>
		</div>
	</div>
</div>
