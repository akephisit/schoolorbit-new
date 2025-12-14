<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { GraduationCap, ArrowLeft } from 'lucide-svelte';

	let email = $state('');
	let password = $state('');
	let rememberMe = $state(false);
	let isLoading = $state(false);

	async function handleSubmit(e: Event) {
		e.preventDefault();
		isLoading = true;

		// Simulate login
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

<div class="min-h-screen bg-background flex items-center justify-center p-4">
	<div class="w-full max-w-md">
		<!-- Back Button -->
		<Button variant="ghost" onclick={goBack} class="mb-6">
			<ArrowLeft class="w-4 h-4 mr-2" />
			กลับหน้าหลัก
		</Button>

		<!-- Card -->
		<div class="bg-card border rounded-lg shadow-sm p-8">
			<!-- Logo & Title -->
			<div class="text-center mb-8">
				<div class="w-16 h-16 bg-primary rounded-lg flex items-center justify-center mx-auto mb-4">
					<GraduationCap class="w-8 h-8 text-primary-foreground" />
				</div>
				<h1 class="text-2xl font-bold text-foreground mb-2">เข้าสู่ระบบ</h1>
				<p class="text-sm text-muted-foreground">SchoolOrbit - ระบบบริหารจัดการโรงเรียน</p>
			</div>

			<!-- Login Form -->
			<form onsubmit={handleSubmit} class="space-y-6">
				<!-- Email Input -->
				<div class="space-y-2">
					<Label for="email">อีเมล</Label>
					<Input
						type="email"
						id="email"
						bind:value={email}
						placeholder="example@school.ac.th"
						required
					/>
				</div>

				<!-- Password Input -->
				<div class="space-y-2">
					<Label for="password">รหัสผ่าน</Label>
					<Input
						type="password"
						id="password"
						bind:value={password}
						placeholder="••••••••"
						required
					/>
				</div>

				<!-- Remember & Forgot -->
				<div class="flex items-center justify-between text-sm">
					<label class="flex items-center gap-2 cursor-pointer">
						<input type="checkbox" bind:checked={rememberMe} class="w-4 h-4 rounded border-input" />
						<span class="text-muted-foreground">จดจำฉันไว้</span>
					</label>
					<Button variant="link" class="p-0 h-auto text-sm">ลืมรหัสผ่าน?</Button>
				</div>

				<!-- Submit Button -->
				<Button type="submit" class="w-full" disabled={isLoading}>
					{isLoading ? 'กำลังเข้าสู่ระบบ...' : 'เข้าสู่ระบบ'}
				</Button>
			</form>

			<!-- Divider -->
			<div class="relative my-6">
				<div class="absolute inset-0 flex items-center">
					<div class="w-full border-t"></div>
				</div>
				<div class="relative flex justify-center text-sm">
					<span class="px-2 bg-card text-muted-foreground">หรือ</span>
				</div>
			</div>

			<!-- Register Link -->
			<div class="text-center text-sm">
				<span class="text-muted-foreground">ยังไม่มีบัญชี?</span>
				<Button variant="link" class="p-0 h-auto ml-1">ลงทะเบียน</Button>
			</div>
		</div>
	</div>
</div>
