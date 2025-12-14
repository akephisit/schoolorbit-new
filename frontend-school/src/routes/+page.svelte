<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from 'bits-ui';
	import { GraduationCap, BookOpen, Users, BarChart3, ArrowRight, Check } from 'lucide-svelte';

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

<div class="min-h-screen bg-white">
	<!-- Navbar -->
	<nav class="bg-white border-b border-gray-200 sticky top-0 z-50">
		<div class="container mx-auto px-6 py-4">
			<div class="flex items-center justify-between">
				<div class="flex items-center space-x-3">
					<div class="w-10 h-10 bg-blue-600 rounded-lg flex items-center justify-center shadow-sm">
						<GraduationCap class="w-6 h-6 text-white" />
					</div>
					<span class="text-gray-900 text-xl font-semibold">SchoolOrbit</span>
				</div>
				<Button.Root
					onclick={handleLogin}
					class="px-5 py-2 bg-blue-600 text-white rounded-lg font-medium hover:bg-blue-700 transition cursor-pointer border-0"
				>
					เข้าสู่ระบบ
				</Button.Root>
			</div>
		</div>
	</nav>

	<!-- Hero Section -->
	<section class="container mx-auto px-6 py-20 md:py-28">
		<div class="max-w-4xl mx-auto text-center">
			<div
				class="transform transition-all duration-700 {mounted
					? 'translate-y-0 opacity-100'
					: 'translate-y-10 opacity-0'}"
			>
				<h1 class="text-4xl md:text-6xl font-bold text-gray-900 mb-6 leading-tight">
					ระบบบริหารจัดการโรงเรียน<br />ยุคดิจิทัล
				</h1>
				<p class="text-lg md:text-xl text-gray-600 mb-10 max-w-2xl mx-auto">
					เพิ่มประสิทธิภาพการทำงาน ลดเวลาธุรการ<br class="hidden md:block" />
					ด้วยระบบที่ออกแบบมาเพื่อโรงเรียนไทยโดยเฉพาะ
				</p>

				<div class="flex flex-col sm:flex-row gap-4 justify-center items-center">
					<Button.Root
						onclick={handleLogin}
						class="px-8 py-3.5 bg-blue-600 text-white rounded-lg font-medium hover:bg-blue-700 transition cursor-pointer border-0 flex items-center gap-2"
					>
						เริ่มต้นใช้งาน
						<ArrowRight class="w-4 h-4" />
					</Button.Root>
					<Button.Root
						onclick={handleLearnMore}
						class="px-8 py-3.5 bg-white text-gray-700 rounded-lg font-medium border-2 border-gray-200 hover:border-gray-300 transition cursor-pointer"
					>
						ดูรายละเอียด
					</Button.Root>
				</div>
			</div>
		</div>
	</section>

	<!-- Features Section -->
	<section id="features" class="bg-gray-50 py-20">
		<div class="container mx-auto px-6">
			<div class="text-center mb-16">
				<h2 class="text-3xl md:text-4xl font-bold text-gray-900 mb-4">
					ฟีเจอร์ครบครัน เพื่อการบริหารที่มีประสิทธิภาพ
				</h2>
				<p class="text-gray-600 text-lg">ตอบโจทย์ทุกความต้องการของโรงเรียนสมัยใหม่</p>
			</div>

			<div class="grid md:grid-cols-3 gap-8 max-w-6xl mx-auto">
				<!-- Feature Card 1 -->
				<div
					class="bg-white rounded-xl p-8 border border-gray-200 hover:shadow-lg transition group"
				>
					<div
						class="w-14 h-14 bg-blue-50 rounded-lg flex items-center justify-center mb-6 group-hover:bg-blue-100 transition"
					>
						<BookOpen class="w-7 h-7 text-blue-600" />
					</div>
					<h3 class="text-xl font-semibold text-gray-900 mb-3">จัดการนักเรียน</h3>
					<p class="text-gray-600 leading-relaxed">
						บันทึกข้อมูลนักเรียนอย่างเป็นระบบ ติดตามผลการเรียน และจัดการทะเบียนได้อย่างครบถ้วน
					</p>
				</div>

				<!-- Feature Card 2 -->
				<div
					class="bg-white rounded-xl p-8 border border-gray-200 hover:shadow-lg transition group"
				>
					<div
						class="w-14 h-14 bg-green-50 rounded-lg flex items-center justify-center mb-6 group-hover:bg-green-100 transition"
					>
						<Users class="w-7 h-7 text-green-600" />
					</div>
					<h3 class="text-xl font-semibold text-gray-900 mb-3">บริหารบุคลากร</h3>
					<p class="text-gray-600 leading-relaxed">
						จัดการข้อมูลครูและเจ้าหน้าที่ พร้อมระบบติดตามการทำงานและประเมินผล
					</p>
				</div>

				<!-- Feature Card 3 -->
				<div
					class="bg-white rounded-xl p-8 border border-gray-200 hover:shadow-lg transition group"
				>
					<div
						class="w-14 h-14 bg-amber-50 rounded-lg flex items-center justify-center mb-6 group-hover:bg-amber-100 transition"
					>
						<BarChart3 class="w-7 h-7 text-amber-600" />
					</div>
					<h3 class="text-xl font-semibold text-gray-900 mb-3">รายงานและวิเคราะห์</h3>
					<p class="text-gray-600 leading-relaxed">
						สร้างรายงานได้ทันที ดูสถิติแบบเรียลไทม์ ช่วยตัดสินใจบริหารได้อย่างมีข้อมูล
					</p>
				</div>
			</div>
		</div>
	</section>

	<!-- Benefits Section -->
	<section class="py-20">
		<div class="container mx-auto px-6">
			<div class="max-w-6xl mx-auto grid md:grid-cols-2 gap-16 items-center">
				<div>
					<h2 class="text-3xl font-bold text-gray-900 mb-6">ทำไมโรงเรียนต้องเลือก SchoolOrbit?</h2>
					<div class="space-y-4">
						<div class="flex gap-3">
							<div
								class="w-6 h-6 bg-blue-100 rounded-full flex items-center justify-center flex-shrink-0 mt-0.5"
							>
								<Check class="w-4 h-4 text-blue-600" />
							</div>
							<div>
								<h4 class="font-semibold text-gray-900 mb-1">ใช้งานง่าย ไม่ซับซ้อน</h4>
								<p class="text-gray-600">ออกแบบ UI/UX ให้เรียบง่าย เข้าใจง่าย ใช้งานได้ทันที</p>
							</div>
						</div>
						<div class="flex gap-3">
							<div
								class="w-6 h-6 bg-blue-100 rounded-full flex items-center justify-center flex-shrink-0 mt-0.5"
							>
								<Check class="w-4 h-4 text-blue-600" />
							</div>
							<div>
								<h4 class="font-semibold text-gray-900 mb-1">ปลอดภัย เชื่อถือได้</h4>
								<p class="text-gray-600">รักษาความปลอดภัยข้อมูลด้วยมาตรฐานระดับสากล</p>
							</div>
						</div>
						<div class="flex gap-3">
							<div
								class="w-6 h-6 bg-blue-100 rounded-full flex items-center justify-center flex-shrink-0 mt-0.5"
							>
								<Check class="w-4 h-4 text-blue-600" />
							</div>
							<div>
								<h4 class="font-semibold text-gray-900 mb-1">รองรับทุกอุปกรณ์</h4>
								<p class="text-gray-600">ใช้งานได้ทั้งคอมพิวเตอร์ แท็บเล็ต และมือถือ</p>
							</div>
						</div>
						<div class="flex gap-3">
							<div
								class="w-6 h-6 bg-blue-100 rounded-full flex items-center justify-center flex-shrink-0 mt-0.5"
							>
								<Check class="w-4 h-4 text-blue-600" />
							</div>
							<div>
								<h4 class="font-semibold text-gray-900 mb-1">สนับสนุนตลอด 24 ชั่วโมง</h4>
								<p class="text-gray-600">ทีมงานพร้อมช่วยเหลือทุกเมื่อที่คุณต้องการ</p>
							</div>
						</div>
					</div>
				</div>
				<div class="bg-gradient-to-br from-blue-50 to-blue-100 rounded-2xl p-12">
					<div class="grid grid-cols-2 gap-8 text-center">
						<div>
							<div class="text-4xl font-bold text-blue-600 mb-2">1,000+</div>
							<div class="text-gray-700">โรงเรียน</div>
						</div>
						<div>
							<div class="text-4xl font-bold text-green-600 mb-2">99.9%</div>
							<div class="text-gray-700">Uptime</div>
						</div>
						<div>
							<div class="text-4xl font-bold text-amber-600 mb-2">24/7</div>
							<div class="text-gray-700">Support</div>
						</div>
						<div>
							<div class="text-4xl font-bold text-purple-600 mb-2">100%</div>
							<div class="text-gray-700">Cloud</div>
						</div>
					</div>
				</div>
			</div>
		</div>
	</section>

	<!-- CTA Section -->
	<section class="bg-blue-600 py-20">
		<div class="container mx-auto px-6">
			<div class="text-center max-w-3xl mx-auto">
				<h2 class="text-3xl md:text-4xl font-bold text-white mb-6">
					พร้อมยกระดับการบริหารโรงเรียนของคุณ?
				</h2>
				<p class="text-lg text-blue-100 mb-8">
					เริ่มต้นใช้งานฟรี ไม่ต้องผูกบัตรเครดิต ทดลองใช้ได้ทันที
				</p>
				<Button.Root
					onclick={handleLogin}
					class="px-10 py-4 bg-white text-blue-600 rounded-lg font-semibold hover:bg-gray-50 transition cursor-pointer border-0 inline-flex items-center gap-2"
				>
					เริ่มต้นใช้งานฟรี
					<ArrowRight class="w-5 h-5" />
				</Button.Root>
			</div>
		</div>
	</section>

	<!-- Footer -->
	<footer class="bg-gray-50 border-t border-gray-200">
		<div class="container mx-auto px-6 py-12">
			<div class="flex flex-col md:flex-row justify-between items-center gap-4">
				<div class="flex items-center space-x-3">
					<div class="w-8 h-8 bg-blue-600 rounded-lg flex items-center justify-center">
						<GraduationCap class="w-5 h-5 text-white" />
					</div>
					<span class="text-gray-900 text-lg font-semibold">SchoolOrbit</span>
				</div>
				<div class="text-center md:text-left">
					<p class="text-gray-600">© 2025 SchoolOrbit - ระบบบริหารจัดการโรงเรียน</p>
					<p class="text-sm text-gray-500 mt-1">พัฒนาเพื่อการศึกษาไทย</p>
				</div>
			</div>
		</div>
	</footer>
</div>
