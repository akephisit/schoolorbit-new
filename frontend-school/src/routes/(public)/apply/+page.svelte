<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getPublicRounds,
		type AdmissionRound,
		roundStatusLabel,
		roundStatusColor
	} from '$lib/api/admission';
	import { getPublicSchoolInfo, type PublicSchoolInfo } from '$lib/api/school';
	import { Button } from '$lib/components/ui/button';
	import { GraduationCap, CalendarDays, ArrowRight, Search } from 'lucide-svelte';

	let loadingRounds = $state(true);
	let publicRounds: AdmissionRound[] = $state([]);
	let schoolInfo = $state<PublicSchoolInfo>({});

	onMount(async () => {
		try {
			[publicRounds, schoolInfo] = await Promise.all([getPublicRounds(), getPublicSchoolInfo()]);
		} catch (e) {
			console.error('Failed to load public rounds', e);
		} finally {
			loadingRounds = false;
		}
	});
</script>

<svelte:head>
	<title>ระบบรับสมัครนักเรียน - SchoolOrbit</title>
</svelte:head>

<div
	class="min-h-screen bg-gradient-to-br from-blue-50 to-indigo-100 flex items-start justify-center py-12 px-4"
>
	<div class="w-full max-w-2xl space-y-6">
		<!-- Header -->
		<div class="text-center">
			{#if schoolInfo.logoUrl}
				<img
					src={schoolInfo.logoUrl}
					alt="school logo"
					class="w-24 h-24 object-contain mx-auto mb-4"
				/>
			{:else}
				<div class="inline-flex p-3 bg-white rounded-2xl shadow-md mb-4">
					<GraduationCap class="w-10 h-10 text-blue-600" />
				</div>
			{/if}
			<h1 class="text-2xl font-bold text-gray-900">ระบบรับสมัครนักเรียน</h1>
			<p class="text-gray-500 mt-1 text-sm">
				กรุณาเลือกรอบรับสมัครที่ท่านสนใจ หรือตรวจสอบผลการสมัครเดิม
			</p>
		</div>

		<!-- Check Status Button -->
		<div
			class="bg-white rounded-2xl shadow-sm border border-indigo-100 p-5 flex flex-col sm:flex-row items-center justify-between gap-4"
		>
			<div>
				<h2 class="text-lg font-semibold text-gray-800 flex items-center gap-2">
					<Search class="w-5 h-5 text-indigo-600" /> ตรวจสอบผลการสมัคร
				</h2>
				<p class="text-sm text-gray-500 mt-1">สำหรับกรอกเลขบัตรประชาชนเพื่อเช็คผลคะแนนและมอบตัว</p>
			</div>
			<Button
				href="/apply/status"
				variant="outline"
				class="border-indigo-200 text-indigo-700 hover:bg-indigo-50 shrink-0"
			>
				ตรวจสอบผล / มอบตัว
			</Button>
		</div>

		{#if loadingRounds}
			<div class="flex justify-center py-6">
				<div class="animate-pulse flex items-center justify-center space-x-2 text-blue-600">
					<div class="w-2 h-2 rounded-full bg-blue-600"></div>
					<div class="w-2 h-2 rounded-full bg-blue-600"></div>
					<div class="w-2 h-2 rounded-full bg-blue-600"></div>
				</div>
			</div>
		{:else if publicRounds.length > 0}
			<!-- Rounds List -->
			<div class="space-y-4 pt-4">
				<h2 class="text-lg font-semibold text-gray-800 flex items-center gap-2">
					<GraduationCap class="w-5 h-5 text-blue-600" /> รอบรับสมัครทั้งหมด
				</h2>
				<div class="grid gap-4">
					{#each publicRounds as r (r.id)}
						<div
							class="bg-white rounded-2xl shadow-sm border border-blue-100 p-5 hover:shadow-md transition-shadow"
						>
							<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
								<div class="space-y-1">
									<div class="flex items-center gap-2 flex-wrap">
										<h3 class="font-bold text-gray-900 text-lg">{r.name}</h3>
										<span
											class="text-xs px-2 py-0.5 rounded-full font-medium {roundStatusColor[
												r.status
											] ?? 'bg-gray-100 text-gray-700'}"
										>
											{roundStatusLabel[r.status] ?? r.status}
										</span>
									</div>
									<p class="text-sm text-gray-600 flex items-center gap-1.5">
										<CalendarDays class="w-4 h-4 text-blue-500" />
										รับสมัคร {new Date(r.applyStartDate).toLocaleDateString('th-TH', {
											month: 'short',
											day: 'numeric'
										})} - {new Date(r.applyEndDate).toLocaleDateString('th-TH', {
											month: 'short',
											day: 'numeric',
											year: 'numeric'
										})}
									</p>
								</div>

								{#if r.status === 'open'}
									<Button
										href="/apply/{r.id}"
										class="bg-blue-600 hover:bg-blue-700 text-white shrink-0 group"
									>
										สมัครเรียน <ArrowRight
											class="w-4 h-4 ml-2 group-hover:translate-x-1 transition-transform"
										/>
									</Button>
								{:else}
									<Button
										href="/apply/status"
										variant="outline"
										class="border-indigo-200 text-indigo-700 hover:bg-indigo-50 shrink-0"
									>
										ตรวจสอบผล <ArrowRight class="w-4 h-4 ml-1" />
									</Button>
								{/if}
							</div>
						</div>
					{/each}
				</div>
			</div>
		{:else}
			<!-- Empty state -->
			<div
				class="bg-white rounded-2xl shadow-sm border border-gray-100 p-8 text-center text-gray-500 mt-4"
			>
				<p>ขณะนี้ยังไม่มีรอบที่เปิดแสดง</p>
			</div>
		{/if}
	</div>
</div>
