<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		getAdmissionPeriod,
		updateAdmissionPeriod,
		PERIOD_STATUS_LABELS,
		type AdmissionPeriod,
		type RequiredDocument,
		type AdmissionStatus
	} from '$lib/api/admission';
	import { getAcademicStructure, type AcademicStructureData } from '$lib/api/academic';
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
	const { periodId } = data;

	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let loading = $state(true);
	let submitting = $state(false);

	// Form
	let name = $state('');
	let description = $state('');
	let openDate = $state('');
	let closeDate = $state('');
	let announcementDate = $state('');
	let confirmationDeadline = $state('');
	let status = $state<AdmissionStatus>('draft');
	let capacityPerClass = $state('');
	let totalCapacity = $state('');
	let waitlistCapacity = $state('');
	let applicationFee = $state('');
	let requiredDocs = $state<RequiredDocument[]>([]);
	let newDocKey = $state('');
	let newDocLabel = $state('');

	const statusOptions: { value: AdmissionStatus; label: string }[] = [
		{ value: 'draft', label: 'ร่าง' },
		{ value: 'open', label: 'เปิดรับสมัคร' },
		{ value: 'closed', label: 'ปิดรับสมัคร' },
		{ value: 'announced', label: 'ประกาศผลแล้ว' },
		{ value: 'done', label: 'เสร็จสิ้น' }
	];

	async function loadData() {
		try {
			const [periodRes, strRes] = await Promise.all([
				getAdmissionPeriod(periodId),
				getAcademicStructure()
			]);
			structure = strRes.data;

			const p = periodRes.data;
			name = p.name;
			description = p.description || '';
			openDate = p.open_date;
			closeDate = p.close_date;
			announcementDate = p.announcement_date || '';
			confirmationDeadline = p.confirmation_deadline || '';
			status = p.status;
			capacityPerClass = p.capacity_per_class?.toString() || '';
			totalCapacity = p.total_capacity?.toString() || '';
			waitlistCapacity = p.waitlist_capacity?.toString() || '';
			applicationFee = p.application_fee?.toString() || '';
			requiredDocs = Array.isArray(p.required_documents) ? p.required_documents : [];
		} catch {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
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
		if (!name || !openDate || !closeDate) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็น');
			return;
		}
		submitting = true;
		try {
			await updateAdmissionPeriod(periodId, {
				name,
				description: description || undefined,
				open_date: openDate,
				close_date: closeDate,
				announcement_date: announcementDate || undefined,
				confirmation_deadline: confirmationDeadline || undefined,
				status,
				capacity_per_class: capacityPerClass ? parseInt(capacityPerClass) : undefined,
				total_capacity: totalCapacity ? parseInt(totalCapacity) : undefined,
				waitlist_capacity: waitlistCapacity ? parseInt(waitlistCapacity) : undefined,
				required_documents: requiredDocs,
				application_fee: applicationFee ? parseFloat(applicationFee) : undefined
			});
			toast.success('บันทึกข้อมูลเรียบร้อยแล้ว');
			goto(`/staff/academic/admission/${periodId}`);
		} catch (e: any) {
			toast.error(e.message || 'บันทึกไม่สำเร็จ');
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
	<div class="flex items-center gap-3">
		<Button variant="ghost" size="icon" href="/staff/academic/admission/{periodId}">
			<ArrowLeft class="h-4 w-4" />
		</Button>
		<div>
			<h1 class="flex items-center gap-2 text-2xl font-bold">
				<ClipboardList class="h-6 w-6 text-primary" />
				แก้ไขรอบรับสมัคร
			</h1>
		</div>
	</div>

	{#if loading}
		<div class="flex h-48 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else}
		<div class="space-y-6">
			<Card.Root>
				<Card.Header><Card.Title>ข้อมูลพื้นฐาน</Card.Title></Card.Header>
				<Card.Content class="grid gap-4 sm:grid-cols-2">
					<div class="grid gap-2 sm:col-span-2">
						<Label for="name">ชื่อรอบรับสมัคร <span class="text-red-500">*</span></Label>
						<Input id="name" bind:value={name} />
					</div>
					<div class="grid gap-2 sm:col-span-2">
						<Label for="desc">คำอธิบาย</Label>
						<Textarea id="desc" bind:value={description} rows={3} />
					</div>
					<div class="grid gap-2">
						<Label>สถานะ</Label>
						<Select.Root type="single" bind:value={status}>
							<Select.Trigger class="w-full">{PERIOD_STATUS_LABELS[status]}</Select.Trigger>
							<Select.Content>
								{#each statusOptions as opt}
									<Select.Item value={opt.value}>{opt.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
				</Card.Content>
			</Card.Root>

			<Card.Root>
				<Card.Header><Card.Title>ช่วงเวลา</Card.Title></Card.Header>
				<Card.Content class="grid gap-4 sm:grid-cols-2">
					<div class="grid gap-2">
						<Label for="od">วันเปิดรับสมัคร <span class="text-red-500">*</span></Label>
						<Input id="od" type="date" bind:value={openDate} />
					</div>
					<div class="grid gap-2">
						<Label for="cd">วันปิดรับสมัคร <span class="text-red-500">*</span></Label>
						<Input id="cd" type="date" bind:value={closeDate} />
					</div>
					<div class="grid gap-2">
						<Label for="ad">วันประกาศผล</Label>
						<Input id="ad" type="date" bind:value={announcementDate} />
					</div>
					<div class="grid gap-2">
						<Label for="co">Deadline ยืนยันสิทธิ์</Label>
						<Input id="co" type="date" bind:value={confirmationDeadline} />
					</div>
				</Card.Content>
			</Card.Root>

			<Card.Root>
				<Card.Header><Card.Title>จำนวนที่รับ</Card.Title></Card.Header>
				<Card.Content class="grid gap-4 sm:grid-cols-3">
					<div class="grid gap-2">
						<Label>รับทั้งหมด</Label>
						<Input type="number" bind:value={totalCapacity} min="0" />
					</div>
					<div class="grid gap-2">
						<Label>ต่อห้อง</Label>
						<Input type="number" bind:value={capacityPerClass} min="0" />
					</div>
					<div class="grid gap-2">
						<Label>สำรอง</Label>
						<Input type="number" bind:value={waitlistCapacity} min="0" />
					</div>
				</Card.Content>
			</Card.Root>

			<Card.Root>
				<Card.Header><Card.Title>เอกสารที่ต้องใช้</Card.Title></Card.Header>
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
								class="h-7 w-7 text-red-400"
								onclick={() => removeDoc(i)}
							>
								<Trash2 class="h-3.5 w-3.5" />
							</Button>
						</div>
					{/each}
					<div class="flex gap-2">
						<Input bind:value={newDocKey} placeholder="รหัส" class="w-36" />
						<Input bind:value={newDocLabel} placeholder="ชื่อเอกสาร" class="flex-1" />
						<Button variant="outline" size="sm" onclick={addDoc}><Plus class="h-4 w-4" /></Button>
					</div>
				</Card.Content>
			</Card.Root>

			<Card.Root>
				<Card.Header><Card.Title>ค่าธรรมเนียม</Card.Title></Card.Header>
				<Card.Content>
					<div class="max-w-xs">
						<Label>จำนวนเงิน (บาท)</Label>
						<Input class="mt-2" type="number" bind:value={applicationFee} min="0" step="0.01" />
					</div>
				</Card.Content>
			</Card.Root>

			<div class="flex justify-end gap-3">
				<Button variant="outline" href="/staff/academic/admission/{periodId}">ยกเลิก</Button>
				<Button onclick={handleSubmit} disabled={submitting} class="min-w-32">
					{#if submitting}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
					บันทึกการแก้ไข
				</Button>
			</div>
		</div>
	{/if}
</div>
