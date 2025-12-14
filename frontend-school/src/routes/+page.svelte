<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from 'bits-ui';

	let mounted = $state(false);

	onMount(() => {
		mounted = true;
	});

	function handleLogin() {
		window.location.href = '/login';
	}

	function handleLearnMore() {
		const featuresSection = document.querySelector('#features');
		featuresSection?.scrollIntoView({ behavior: 'smooth' });
	}
</script>

<svelte:head>
	<title>SchoolOrbit - ระบบบริหารจัดการโรงเรียน</title>
	<meta name="description" content="ระบบบริหารจัดการโรงเรียนที่ทันสมัย ใช้งานง่าย" />
</svelte:head>

<div class="min-h-screen bg-gradient-to-br from-indigo-500 via-purple-500 to-pink-500">
	<!-- Navbar -->
	<nav class="bg-white/10 backdrop-blur-md border-b border-white/20 sticky top-0 z-50">
		<div class="container mx-auto px-6 py-4">
			<div class="flex items-center justify-between">
				<div class="flex items-center space-x-2">
					<div
						class="w-10 h-10 bg-white rounded-lg flex items-center justify-center shadow-lg transform transition hover:scale-110 hover:rotate-6"
					>
						<span class="text-2xl">🎓</span>
					</div>
					<span class="text-white text-2xl font-bold">SchoolOrbit</span>
				</div>
				<Button.Root
					onclick={handleLogin}
					class="px-6 py-2.5 bg-white text-indigo-600 rounded-lg font-medium hover:bg-indigo-50 transition shadow-lg hover:shadow-xl transform hover:-translate-y-0.5 cursor-pointer border-0"
				>
					เข้าสู่ระบบ
				</Button.Root>
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
				<h1 class="text-5xl md:text-7xl font-bold text-white mb-6 leading-tight animate-fade-in">
					ระบบบริหารจัดการ<br />โรงเรียนยุคใหม่
				</h1>
				<p class="text-xl md:text-2xl text-white/90 mb-10 font-light">
					จัดการข้อมูลนักเรียน ครู และงานธุรการได้อย่างมีประสิทธิภาพ<br />
					ด้วยระบบคลาวด์ที่ปลอดภัยและใช้งานง่าย
				</p>

				<div class="flex flex-col sm:flex-row gap-4 justify-center items-center">
					<Button.Root
						onclick={handleLogin}
						class="px-8 py-4 bg-white text-indigo-600 rounded-xl font-semibold text-lg hover:bg-indigo-50 transition shadow-2xl hover:shadow-3xl transform hover:scale-105 cursor-pointer border-0"
					>
						เริ่มต้นใช้งาน →
					</Button.Root>
					<Button.Root
						onclick={handleLearnMore}
						class="px-8 py-4 bg-white/20 backdrop-blur-sm text-white rounded-xl font-semibold text-lg hover:bg-white/30 transition border-2 border-white/30 cursor-pointer"
					>
						ดูรายละเอียด
					</Button.Root>
				</div>
			</div>
		</div>
	</section>

	<!-- Features Section -->
	<section id="features" class="container mx-auto px-6 pb-20">
		<div class="text-center mb-12">
			<h2 class="text-4xl font-bold text-white mb-4">ฟีเจอร์เด่น</h2>
			<p class="text-white/80 text-lg">ครบครันทุกการใช้งาน ตอบโจทย์ทุกความต้องการของโรงเรียน</p>
		</div>

		<div class="grid md:grid-cols-3 gap-8">
			<!-- Feature Card 1 -->
			<div
				class="bg-white/10 backdrop-blur-md rounded-2xl p-8 border border-white/20 hover:bg-white/20 transition-all duration-300 transform hover:-translate-y-2 hover:shadow-2xl group"
			>
				<div
					class="w-16 h-16 bg-gradient-to-br from-yellow-400 to-orange-500 rounded-xl flex items-center justify-center mb-6 shadow-lg group-hover:scale-110 transition-transform"
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
				class="bg-white/10 backdrop-blur-md rounded-2xl p-8 border border-white/20 hover:bg-white/20 transition-all duration-300 transform hover:-translate-y-2 hover:shadow-2xl group"
			>
				<div
					class="w-16 h-16 bg-gradient-to-br from-green-400 to-emerald-500 rounded-xl flex items-center justify-center mb-6 shadow-lg group-hover:scale-110 transition-transform"
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
				class="bg-white/10 backdrop-blur-md rounded-2xl p-8 border border-white/20 hover:bg-white/20 transition-all duration-300 transform hover:-translate-y-2 hover:shadow-2xl group"
			>
				<div
					class="w-16 h-16 bg-gradient-to-br from-blue-400 to-cyan-500 rounded-xl flex items-center justify-center mb-6 shadow-lg group-hover:scale-110 transition-transform"
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

	<!-- Stats Section -->
	<section class="container mx-auto px-6 pb-20">
		<div class="bg-white/10 backdrop-blur-md rounded-3xl p-12 border border-white/20">
			<div class="grid md:grid-cols-3 gap-8 text-center">
				<div>
					<div class="text-5xl font-bold text-white mb-2">1,000+</div>
					<div class="text-white/80">โรงเรียนที่ใช้งาน</div>
				</div>
				<div>
					<div class="text-5xl font-bold text-white mb-2">99.9%</div>
					<div class="text-white/80">Uptime</div>
				</div>
				<div>
					<div class="text-5xl font-bold text-white mb-2">24/7</div>
					<div class="text-white/80">การสนับสนุน</div>
				</div>
			</div>
		</div>
	</section>

	<!-- CTA Section -->
	<section class="container mx-auto px-6 pb-20">
		<div class="text-center max-w-3xl mx-auto">
			<h2 class="text-4xl font-bold text-white mb-6">พร้อมเริ่มต้นแล้วหรือยัง?</h2>
			<p class="text-xl text-white/90 mb-8">
				เริ่มใช้งาน SchoolOrbit วันนี้ และสัมผัสประสบการณ์การจัดการโรงเรียนแบบใหม่
			</p>
			<Button.Root
				onclick={handleLogin}
				class="px-10 py-5 bg-white text-indigo-600 rounded-xl font-bold text-xl hover:bg-indigo-50 transition shadow-2xl hover:shadow-3xl transform hover:scale-110 cursor-pointer border-0"
			>
				เริ่มต้นฟรี →
			</Button.Root>
		</div>
	</section>

	<!-- Footer -->
	<footer class="bg-white/10 backdrop-blur-md border-t border-white/20">
		<div class="container mx-auto px-6 py-8">
			<div class="text-center text-white/70">
				<p class="font-light">© 2025 SchoolOrbit - ระบบบริหารจัดการโรงเรียน</p>
				<p class="text-sm mt-2">พัฒนาด้วย ❤️ เพื่อการศึกษาไทย</p>
			</div>
		</div>
	</footer>
</div>

<style>
	@keyframes fade-in {
		from {
			opacity: 0;
			transform: translateY(20px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.animate-fade-in {
		animation: fade-in 0.8s ease-out;
	}
</style>
