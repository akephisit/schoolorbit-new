<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { authStore } from '$lib/stores/auth.svelte';

	// Redirect to dashboard if already authenticated
	onMount(() => {
		if (authStore.isAuthenticated) {
			goto('/dashboard');
		}
	});

	// Animation state using Svelte 5 runes
	let isVisible = $state(false);

	onMount(() => {
		// Trigger entrance animation
		setTimeout(() => {
			isVisible = true;
		}, 100);
	});

	function handleGetStarted() {
		goto('/login');
	}
</script>

<svelte:head>
	<title>SchoolOrbit Admin - ระบบบริหารจัดการโรงเรียน</title>
	<meta name="description" content="ระบบบริหารจัดการโรงเรียนแบบครบวงจร พร้อมความปลอดภัยสูง" />
</svelte:head>

<div class="min-h-screen bg-gradient-to-br from-blue-50 via-white to-purple-50 overflow-hidden">
	<!-- Animated background elements -->
	<div class="absolute inset-0 overflow-hidden pointer-events-none">
		<div class="absolute top-20 left-10 w-72 h-72 bg-blue-200 rounded-full mix-blend-multiply filter blur-xl opacity-20 animate-blob"></div>
		<div class="absolute top-40 right-10 w-72 h-72 bg-purple-200 rounded-full mix-blend-multiply filter blur-xl opacity-20 animate-blob animation-delay-2000"></div>
		<div class="absolute -bottom-8 left-20 w-72 h-72 bg-pink-200 rounded-full mix-blend-multiply filter blur-xl opacity-20 animate-blob animation-delay-4000"></div>
	</div>

	<!-- Main content -->
	<div class="relative z-10 flex flex-col items-center justify-center min-h-screen px-4 sm:px-6 lg:px-8">
		<div class="max-w-5xl w-full space-y-12 text-center"
			class:opacity-0={!isVisible}
			class:translate-y-10={!isVisible}
			class:opacity-100={isVisible}
			class:translate-y-0={isVisible}
			style="transition: all 0.8s cubic-bezier(0.4, 0, 0.2, 1);">
			
			<!-- Logo / Icon -->
			<div class="flex justify-center">
				<div class="relative">
					<div class="absolute inset-0 bg-gradient-to-r from-blue-600 to-purple-600 rounded-2xl blur-lg opacity-30 animate-pulse"></div>
					<div class="relative bg-gradient-to-r from-blue-600 to-purple-600 p-6 rounded-2xl shadow-2xl">
						<svg class="w-16 h-16 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"></path>
						</svg>
					</div>
				</div>
			</div>

			<!-- Main heading -->
			<div class="space-y-4">
				<h1 class="text-5xl sm:text-6xl lg:text-7xl font-bold">
					<span class="bg-gradient-to-r from-blue-600 via-purple-600 to-pink-600 bg-clip-text text-transparent">
						SchoolOrbit
					</span>
					<span class="block text-4xl sm:text-5xl lg:text-6xl mt-2 text-gray-900">
						Admin Portal
					</span>
				</h1>
				<p class="text-xl sm:text-2xl text-gray-600 max-w-3xl mx-auto leading-relaxed">
					ระบบบริหารจัดการโรงเรียนแบบครบวงจร
					<span class="block mt-2 text-lg text-gray-500">
						พร้อมความปลอดภัยระดับสูงตามมาตรฐาน PDPA
					</span>
				</p>
			</div>

			<!-- Features grid -->
			<div class="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-4xl mx-auto mt-12">
				<!-- Feature 1 -->
				<div class="group bg-white/80 backdrop-blur-sm p-6 rounded-2xl shadow-lg hover:shadow-2xl transition-all duration-300 hover:-translate-y-1 border border-gray-100">
					<div class="w-12 h-12 bg-blue-100 rounded-xl flex items-center justify-center mb-4 group-hover:bg-blue-200 transition-colors">
						<svg class="w-6 h-6 text-blue-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"></path>
						</svg>
					</div>
					<h3 class="text-lg font-semibold text-gray-900 mb-2">ความปลอดภัยสูง</h3>
					<p class="text-sm text-gray-600">ระบบเข้ารหัสข้อมูลส่วนบุคคลตามมาตรฐาน PDPA</p>
				</div>

				<!-- Feature 2 -->
				<div class="group bg-white/80 backdrop-blur-sm p-6 rounded-2xl shadow-lg hover:shadow-2xl transition-all duration-300 hover:-translate-y-1 border border-gray-100">
					<div class="w-12 h-12 bg-purple-100 rounded-xl flex items-center justify-center mb-4 group-hover:bg-purple-200 transition-colors">
						<svg class="w-6 h-6 text-purple-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"></path>
						</svg>
					</div>
					<h3 class="text-lg font-semibold text-gray-900 mb-2">รวดเร็วทันใจ</h3>
					<p class="text-sm text-gray-600">สร้างด้วย Rust และ SvelteKit เพื่อประสิทธิภาพสูงสุด</p>
				</div>

				<!-- Feature 3 -->
				<div class="group bg-white/80 backdrop-blur-sm p-6 rounded-2xl shadow-lg hover:shadow-2xl transition-all duration-300 hover:-translate-y-1 border border-gray-100">
					<div class="w-12 h-12 bg-pink-100 rounded-xl flex items-center justify-center mb-4 group-hover:bg-pink-200 transition-colors">
						<svg class="w-6 h-6 text-pink-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"></path>
						</svg>
					</div>
					<h3 class="text-lg font-semibold text-gray-900 mb-2">เชื่อถือได้</h3>
					<p class="text-sm text-gray-600">ออกแบบมาเพื่อความมั่นคงและพร้อมใช้งาน 24/7</p>
				</div>
			</div>

			<!-- CTA Button -->
			<div class="mt-12 space-y-4">
				<button
					onclick={handleGetStarted}
					class="group relative inline-flex items-center justify-center px-8 py-4 text-lg font-semibold text-white transition-all duration-300 ease-in-out bg-gradient-to-r from-blue-600 to-purple-600 rounded-xl shadow-lg hover:shadow-2xl hover:scale-105 active:scale-95"
				>
					<span class="relative flex items-center gap-2">
						เข้าสู่ระบบ
						<svg class="w-5 h-5 transition-transform duration-300 group-hover:translate-x-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7l5 5m0 0l-5 5m5-5H6"></path>
						</svg>
					</span>
					<div class="absolute inset-0 bg-gradient-to-r from-purple-600 to-pink-600 rounded-xl opacity-0 group-hover:opacity-100 transition-opacity duration-300 -z-10"></div>
				</button>
				
				<p class="text-sm text-gray-500">
					ใช้เลขบัตรประชาชนเพื่อเข้าสู่ระบบอย่างปลอดภัย
				</p>
			</div>

			<!-- Stats / Info -->
			<div class="mt-16 pt-8 border-t border-gray-200">
				<div class="grid grid-cols-1 sm:grid-cols-3 gap-8 max-w-3xl mx-auto">
					<div class="text-center">
						<div class="text-3xl font-bold bg-gradient-to-r from-blue-600 to-purple-600 bg-clip-text text-transparent">99.9%</div>
						<div class="text-sm text-gray-600 mt-1">Uptime</div>
					</div>
					<div class="text-center">
						<div class="text-3xl font-bold bg-gradient-to-r from-purple-600 to-pink-600 bg-clip-text text-transparent">PDPA</div>
						<div class="text-sm text-gray-600 mt-1">Compliant</div>
					</div>
					<div class="text-center">
						<div class="text-3xl font-bold bg-gradient-to-r from-pink-600 to-blue-600 bg-clip-text text-transparent">24/7</div>
						<div class="text-sm text-gray-600 mt-1">Support</div>
					</div>
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	@keyframes blob {
		0% {
			transform: translate(0px, 0px) scale(1);
		}
		33% {
			transform: translate(30px, -50px) scale(1.1);
		}
		66% {
			transform: translate(-20px, 20px) scale(0.9);
		}
		100% {
			transform: translate(0px, 0px) scale(1);
		}
	}

	.animate-blob {
		animation: blob 7s infinite;
	}

	.animation-delay-2000 {
		animation-delay: 2s;
	}

	.animation-delay-4000 {
		animation-delay: 4s;
	}
</style>
