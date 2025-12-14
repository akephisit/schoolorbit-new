<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';

	let mounted = $state(false);

	onMount(() => {
		mounted = true;
	});

	function handleLogin() {
		window.location.href = '/login';
	}

	function handleLearnMore() {
		alert('ฟีเจอร์นี้กำลังพัฒนา');
	}
</script>

<svelte:head>
	<title>SchoolOrbit - ระบบจัดการโรงเรียนแบบครบวงจร</title>
	<meta name="description" content="ระบบจัดการโรงเรียนแบบครบวงจร" />
</svelte:head>

<div class="min-h-screen bg-background flex items-center justify-center px-4">
	<div class="max-w-2xl mx-auto text-center">
		<div
			class="transform transition-all duration-700 {mounted
				? 'translate-y-0 opacity-100'
				: 'translate-y-10 opacity-0'}"
		>
			<!-- Logo/Title -->
			<h1 class="text-5xl md:text-6xl font-bold text-foreground mb-4">SchoolOrbit</h1>

			<!-- Subtitle -->
			<p class="text-lg md:text-xl text-muted-foreground mb-8">ระบบจัดการโรงเรียนแบบครบวงจร</p>

			<!-- Buttons -->
			<div class="flex flex-col sm:flex-row gap-4 justify-center mb-8">
				<Button onclick={handleLogin} size="lg" class="font-medium">เข้าสู่ระบบ</Button>
				<Button onclick={handleLearnMore} variant="outline" size="lg" class="font-medium">
					เรียนรู้เพิ่มเติม
				</Button>
			</div>

			<!-- Description -->
			<p class="text-muted-foreground text-sm md:text-base max-w-xl mx-auto leading-relaxed">
				จัดการข้อมูลนักเรียนและครูทั้งหมดได้อย่างมีประสิทธิภาพที่สุดในรูปแบบดิจิทัล
				ในช่วงเวลาที่จำกัด
			</p>
		</div>
	</div>
</div>
