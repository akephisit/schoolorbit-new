<script lang="ts">
	import { onMount } from 'svelte';

	let mounted = $state(false);

	onMount(() => {
		mounted = true;
	});

	function handleLogin() {
		// Navigate to login page (you can implement this later)
		window.location.href = '/login';
	}
</script>

<svelte:head>
	<title>SchoolOrbit - ระบบบริหารจัดการโรงเรียน</title>
	<meta name="description" content="ระบบบริหารจัดการโรงเรียนที่ทันสมัย ใช้งานง่าย" />
</svelte:head>

<div class="min-h-screen bg-gradient-to-br from-indigo-500 via-purple-500 to-pink-500">
	<!-- Navbar -->
	<nav class="bg-white/10 backdrop-blur-md border-b border-white/20">
		<div class="container mx-auto px-6 py-4">
			<div class="flex items-center justify-between">
				<div class="flex items-center space-x-2">
					<div
						class="w-10 h-10 bg-white rounded-lg flex items-center justify-center shadow-lg transform transition hover:scale-110"
					>
						<span class="text-2xl">🎓</span>
					</div>
					<span class="text-white text-2xl font-bold">SchoolOrbit</span>
				</div>
				<button
					onclick={handleLogin}
					class="px-6 py-2.5 bg-white text-indigo-600 rounded-lg font-medium hover:bg-indigo-50 transition shadow-lg hover:shadow-xl transform hover:-translate-y-0.5"
				>
					เข้าสู่ระบบ
				</button>
			</div>
		</div>
	</nav>

	<!-- Hero Section -->
	<section class="container mx-auto px-6 py-20">
		<div class="text-center max-w-4xl mx-auto">
			<div
				class="transform transition-all duration-700 {mounted
					? 'translate-y-0 opacity-100'
					: 'translate-y-10 opacity-0'}"
			>
				<h1 class="text-5xl md:text-7xl font-bold text-white mb-6 leading-tight">
					ระบบบริหารจัดการ<br />โรงเรียนยุคใหม่
				</h1>
				<p class="text-xl md:text-2xl text-white/90 mb-10 font-light">
					จัดการข้อมูลนักเรียน ครู และงานธุรการได้อย่างมีประสิทธิภาพ<br />
					ด้วยระบบคลาวด์ที่ปลอดภัยและใช้งานง่าย
				</p>

				<div class="flex flex-col sm:flex-row gap-4 justify-center items-center">
					<button
						onclick={handleLogin}
						class="px-8 py-4 bg-white text-indigo-600 rounded-xl font-semibold text-lg hover:bg-indigo-50 transition shadow-2xl hover:shadow-3xl transform hover:scale-105"
					>
						เริ่มต้นใช้งาน →
					</button>
					<button
						class="px-8 py-4 bg-white/20 backdrop-blur-sm text-white rounded-xl font-semibold text-lg hover:bg-white/30 transition border-2 border-white/30"
					>
						ดูรายละเอียด
					</button>
				</div>
			</div>
		</div>
	</section>

	<!-- Features Section -->
	<section class="container mx-auto px-6 pb-20">
		<div class="grid md:grid-cols-3 gap-8">
			<!-- Feature Card 1 -->
			<div
				class="bg-white/10 backdrop-blur-md rounded-2xl p-8 border border-white/20 hover:bg-white/20 transition transform hover:-translate-y-2 hover:shadow-2xl"
			>
				<div
					class="w-16 h-16 bg-gradient-to-br from-yellow-400 to-orange-500 rounded-xl flex items-center justify-center mb-6 shadow-lg"
				>
					<span class="text-3xl">📚</span>
				</div>
				<h3 class="text-2xl font-bold text-white mb-4">จัดการนักเรียน</h3>
				<p class="text-white/80 leading-relaxed">
					บันทึกและจัดการข้อมูลนักเรียนได้อย่างครบถ้วน พร้อมระบบรายงานที่ละเอียด
				</p>
			</div>

			<!-- Feature Card 2 -->
			<div
				class="bg-white/10 backdrop-blur-md rounded-2xl p-8 border border-white/20 hover:bg-white/20 transition transform hover:-translate-y-2 hover:shadow-2xl"
			>
				<div
					class="w-16 h-16 bg-gradient-to-br from-green-400 to-emerald-500 rounded-xl flex items-center justify-center mb-6 shadow-lg"
				>
					<span class="text-3xl">👨‍🏫</span>
				</div>
				<h3 class="text-2xl font-bold text-white mb-4">บริหารบุคลากร</h3>
				<p class="text-white/80 leading-relaxed">
					จัดการข้อมูลครูและบุคลากร พร้อมระบบติดตามการทำงานอย่างมีประสิทธิภาพ
				</p>
			</div>

			<!-- Feature Card 3 -->
			<div
				class="bg-white/10 backdrop-blur-md rounded-2xl p-8 border border-white/20 hover:bg-white/20 transition transform hover:-translate-y-2 hover:shadow-2xl"
			>
				<div
					class="w-16 h-16 bg-gradient-to-br from-blue-400 to-cyan-500 rounded-xl flex items-center justify-center mb-6 shadow-lg"
				>
					<span class="text-3xl">📊</span>
				</div>
				<h3 class="text-2xl font-bold text-white mb-4">รายงานแบบเรียลไทม์</h3>
				<p class="text-white/80 leading-relaxed">
					ดูสถิติและรายงานต่างๆ แบบเรียลไทม์ ช่วยให้ตัดสินใจได้อย่างรวดเร็ว
				</p>
			</div>
		</div>
	</section>

	<!-- Footer -->
	<footer class="bg-white/10 backdrop-blur-md border-t border-white/20 mt-20">
		<div class="container mx-auto px-6 py-8">
			<div class="text-center text-white/70">
				<p class="font-light">© 2025 SchoolOrbit - ระบบบริหารจัดการโรงเรียน</p>
				<p class="text-sm mt-2">พัฒนาด้วย ❤️ เพื่อการศึกษาไทย</p>
			</div>
		</div>
	</footer>
</div>

<style>
	/* Additional animations and effects */
	@keyframes float {
		0%, 100% {
			transform: translateY(0px);
		}
		50% {
			transform: translateY(-10px);
		}
	}
	
	/* Custom gradient animation */
	@keyframes gradient {
		0% {
			background-position: 0% 50%;
		}
		50% {
			background-position: 100% 50%;
		}
		100% {
			background-position: 0% 50%;
		}
	}
</style>
