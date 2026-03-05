<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { createAdmissionPeriod, type RequiredDocument } from '$lib/api/admission';
	import {
		getAcademicStructure,
		type AcademicStructureData,
		type GradeLevel
	} from '$lib/api/academic';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Select from '$lib/components/ui/select';
	import Loader2 from 'lucide-svelte/icons/loader-2';
	import ArrowLeft from 'lucide-svelte/icons/arrow-left';
	import Plus from 'lucide-svelte/icons/plus';
	import Trash2 from 'lucide-svelte/icons/trash-2';
	import ClipboardList from 'lucide-svelte/icons/clipboard-list';

	let { data } = $props();

	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let loading = $state(true);
	let submitting = $state(false);

	// Form
	let academicYearId = $state('');
	let name = $state('');
	let description = $state('');
	let openDate = $state('');
	let closeDate = $state('');
	let announcementDate = $state('');
	let confirmationDeadline = $state('');
	let capacityPerClass = $state('');
	let totalCapacity = $state('');
	let waitlistCapacity = $state('');
	let applicationFee = $state('');
	let selectedLevelIds = $state<string[]>([]);
	let requiredDocs = $state<RequiredDocument[]>([
		{ key: 'birth_cert', label: 'สูติบัตร', required: true },
		{ key: 'house_reg', label: 'ทะเบียนบ้าน', required: true },
		{ key: 'transcript', label: 'ปพ.1 หรือผลการเรียน', required: false },
		{ key: 'photo', label: 'รูปถ่ายนักเรียน', required: true }
	]);
	let newDocKey = $state('');
	let newDocLabel = $state('');

	async function loadData() {
		try {
			const res = await getAcademicStructure();
			structure = res.data;
			const activeYear = structure.years.find((y) => y.is_active);
			if (activeYear) {
				academicYearId = activeYear.id;
				// Auto-fill name
				name = `รับสมัครนักเรียน ปีการศึกษา ${activeYear.name}`;
			}
		} catch {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	function toggleLevel(id: string) {
		if (selectedLevelIds.includes(id)) {
			selectedLevelIds = selectedLevelIds.filter((x) => x !== id);
		} else {
			selectedLevelIds = [...selectedLevelIds, id];
		}
	}

	function addDoc() {
		if (!newDocKey.trim() || !newDocLabel.trim()) return;
		requiredDocs = [
			...requiredDocs,
			{ key: newDocKey.trim(), label: newDocLabel.trim(), required: false }
		];
		newDocKey = '';
		newDocLabel = '';
	}

	function removeDoc(index: number) {
		requiredDocs = requiredDocs.filter((_, i) => i !== index);
	}

	async function handleSubmit() {
		if (!academicYearId || !name || !openDate || !closeDate) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็นให้ครบ');
			return;
		}
		if (closeDate <= openDate) {
			toast.error('วันปิดรับสมัครต้องหลังวันเปิดรับสมัคร');
			return;
		}

		submitting = true;
		try {
			await createAdmissionPeriod({
				academic_year_id: academicYearId,
				name,
				description: description || undefined,
				open_date: openDate,
				close_date: closeDate,
				announcement_date: announcementDate || undefined,
				confirmation_deadline: confirmationDeadline || undefined,
				target_grade_level_ids: selectedLevelIds,
				capacity_per_class: capacityPerClass ? parseInt(capacityPerClass) : undefined,
				total_capacity: totalCapacity ? parseInt(totalCapacity) : undefined,
				waitlist_capacity: waitlistCapacity ? parseInt(waitlistCapacity) : undefined,
				required_documents: requiredDocs,
				application_fee: applicationFee ? parseFloat(applicationFee) : undefined
			});
			toast.success('สร้างรอบรับสมัครเรียบร้อยแล้ว');
			goto('/staff/academic/admission');
		} catch (e: any) {
			toast.error(e.message || 'สร้างไม่สำเร็จ');
		} finally {
			submitting = false;
		}
	}

	onMount(loadData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="mx-auto max-w-4xl space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-4">
		<Button variant="ghost" size="icon" href="/staff/academic/admission">
			<ArrowLeft class="h-4 w-4" />
		</Button>
		<div>
			<h1 class="flex items-center gap-2 text-2xl font-bold text-foreground">
				<ClipboardList class="h-6 w-6 text-primary" />
				สร้างรอบรับสมัครใหม่
			</h1>
			<p class="text-sm text-muted-foreground">กำหนดรายละเอียดสำหรับรอบรับสมัครนักเรียน</p>
		</div>
	</div>

	{#if loading}
		<div class="flex h-48 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else}
		<div class="space-y-6">
			<!-- ข้อมูลพื้นฐาน -->
			<Card.Root>
				<Card.Header>
					<Card.Title>ข้อมูลพื้นฐาน</Card.Title>
				</Card.Header>
				<Card.Content class="grid gap-5">
					<div class="grid gap-2">
						<Label for="year">ปีการศึกษา <span class="text-red-500">*</span></Label>
						<Select.Root type="single" bind:value={academicYearId}>
							<Select.Trigger id="year" class="w-full">
								{structure.years.find((y) => y.id === academicYearId)?.name || 'เลือกปีการศึกษา'}
							</Select.Trigger>
							<Select.Content>
								{#each structure.years as year}
									<Select.Item value={year.id}
										>{year.name} {year.is_active ? '(ปัจจุบัน)' : ''}</Select.Item
									>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>

					<div class="grid gap-2">
						<Label for="name">ชื่อรอบรับสมัคร <span class="text-red-500">*</span></Label>
						<Input
							id="name"
							bind:value={name}
							placeholder="เช่น รับสมัครนักเรียน ม.1 ปีการศึกษา 2568"
						/>
					</div>

					<div class="grid gap-2">
						<Label for="desc">คำอธิบาย</Label>
						<Textarea
							id="desc"
							bind:value={description}
							placeholder="รายละเอียดเพิ่มเติมเกี่ยวกับรอบรับสมัครนี้..."
							rows={3}
						/>
					</div>
				</Card.Content>
			</Card.Root>

			<!-- ช่วงเวลา -->
			<Card.Root>
				<Card.Header>
					<Card.Title>ช่วงเวลา</Card.Title>
				</Card.Header>
				<Card.Content class="grid gap-5 sm:grid-cols-2">
					<div class="grid gap-2">
						<Label for="openDate">วันเปิดรับสมัคร <span class="text-red-500">*</span></Label>
						<Input id="openDate" type="date" bind:value={openDate} />
					</div>
					<div class="grid gap-2">
						<Label for="closeDate">วันปิดรับสมัคร <span class="text-red-500">*</span></Label>
						<Input id="closeDate" type="date" bind:value={closeDate} />
					</div>
					<div class="grid gap-2">
						<Label for="annDate">วันประกาศผล</Label>
						<Input id="annDate" type="date" bind:value={announcementDate} />
					</div>
					<div class="grid gap-2">
						<Label for="confDeadline">Deadline ยืนยันสิทธิ์</Label>
						<Input id="confDeadline" type="date" bind:value={confirmationDeadline} />
					</div>
				</Card.Content>
			</Card.Root>

			<!-- ระดับชั้นที่รับสมัคร -->
			<Card.Root>
				<Card.Header>
					<Card.Title>ระดับชั้นที่รับสมัคร</Card.Title>
					<Card.Description
						>เลือกระดับชั้นที่เปิดรับในรอบนี้ (สามารถเลือกได้หลายระดับ)</Card.Description
					>
				</Card.Header>
				<Card.Content>
					<div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
						{#each structure.levels as level}
							<label
								class="flex cursor-pointer items-center gap-2 rounded-lg border p-3 transition-colors hover:bg-muted/50 {selectedLevelIds.includes(
									level.id
								)
									? 'border-primary bg-primary/5'
									: ''}"
							>
								<Checkbox
									checked={selectedLevelIds.includes(level.id)}
									onCheckedChange={() => toggleLevel(level.id)}
								/>
								<span class="text-sm font-medium">{level.short_name || level.name}</span>
							</label>
						{/each}
					</div>
				</Card.Content>
			</Card.Root>

			<!-- จำนวนรับ -->
			<Card.Root>
				<Card.Header>
					<Card.Title>จำนวนที่รับ</Card.Title>
				</Card.Header>
				<Card.Content class="grid gap-5 sm:grid-cols-3">
					<div class="grid gap-2">
						<Label for="totalCap">จำนวนรับทั้งหมด</Label>
						<Input
							id="totalCap"
							type="number"
							bind:value={totalCapacity}
							placeholder="เช่น 120"
							min="0"
						/>
					</div>
					<div class="grid gap-2">
						<Label for="classCap">จำนวนต่อห้อง</Label>
						<Input
							id="classCap"
							type="number"
							bind:value={capacityPerClass}
							placeholder="เช่น 40"
							min="0"
						/>
					</div>
					<div class="grid gap-2">
						<Label for="waitCap">รายชื่อสำรอง</Label>
						<Input
							id="waitCap"
							type="number"
							bind:value={waitlistCapacity}
							placeholder="เช่น 20"
							min="0"
						/>
					</div>
				</Card.Content>
			</Card.Root>

			<!-- เอกสารที่ต้องใช้ -->
			<Card.Root>
				<Card.Header>
					<Card.Title>เอกสารที่ต้องใช้ในการสมัคร</Card.Title>
				</Card.Header>
				<Card.Content class="space-y-3">
					{#each requiredDocs as doc, i}
						<div class="flex items-center gap-3 rounded-lg border bg-muted/30 px-4 py-3">
							<Checkbox
								checked={doc.required}
								onCheckedChange={(v) => {
									requiredDocs = requiredDocs.map((d, idx) =>
										idx === i ? { ...d, required: v as boolean } : d
									);
								}}
							/>
							<span class="flex-1 text-sm font-medium">{doc.label}</span>
							<span class="text-xs text-muted-foreground"
								>{doc.required ? 'บังคับ' : 'ไม่บังคับ'}</span
							>
							<Button
								variant="ghost"
								size="icon"
								class="h-7 w-7 text-red-400 hover:text-red-600"
								onclick={() => removeDoc(i)}
							>
								<Trash2 class="h-3.5 w-3.5" />
							</Button>
						</div>
					{/each}

					<!-- Add new doc -->
					<div class="flex gap-2">
						<Input bind:value={newDocKey} placeholder="รหัส (เช่น extra_cert)" class="w-40" />
						<Input bind:value={newDocLabel} placeholder="ชื่อเอกสาร" class="flex-1" />
						<Button variant="outline" size="sm" onclick={addDoc}>
							<Plus class="h-4 w-4" />
						</Button>
					</div>
				</Card.Content>
			</Card.Root>

			<!-- ค่าธรรมเนียม -->
			<Card.Root>
				<Card.Header>
					<Card.Title>ค่าธรรมเนียมการสมัคร</Card.Title>
				</Card.Header>
				<Card.Content>
					<div class="max-w-xs">
						<div class="grid gap-2">
							<Label for="fee">จำนวนเงิน (บาท)</Label>
							<Input
								id="fee"
								type="number"
								bind:value={applicationFee}
								placeholder="0 = ไม่มีค่าธรรมเนียม"
								min="0"
								step="0.01"
							/>
						</div>
					</div>
				</Card.Content>
			</Card.Root>

			<!-- Submit -->
			<div class="flex justify-end gap-3">
				<Button variant="outline" href="/staff/academic/admission">ยกเลิก</Button>
				<Button onclick={handleSubmit} disabled={submitting} class="min-w-32">
					{#if submitting}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{/if}
					สร้างรอบรับสมัคร
				</Button>
			</div>
		</div>
	{/if}
</div>
