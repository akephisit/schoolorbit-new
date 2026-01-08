<script lang="ts">
	import { onMount } from 'svelte';
	import { consentApi, type ConsentType, type CreateConsentRequest } from '$lib/api/consent';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Label } from '$lib/components/ui/label';
	import * as Card from '$lib/components/ui/card';
	import { LoaderCircle, AlertCircle, CheckCircle2 } from 'lucide-svelte';

	// Props
	interface Props {
		userType?: 'student' | 'staff' | 'parent';
		isMinor?: boolean;
		onComplete?: () => void;
		onSkip?: () => void;
		showSkipButton?: boolean;
	}

	let {
		userType = 'student',
		isMinor = false,
		onComplete,
		onSkip,
		showSkipButton = false
	}: Props = $props();

	// State
	let consentTypes = $state<ConsentType[]>([]);
	let selectedConsents = $state<Set<string>>(new Set());
	let parentGuardianName = $state('');
	let parentRelationship = $state<'father' | 'mother' | 'guardian'>('father');
	let loading = $state(true);
	let submitting = $state(false);
	let error = $state<string | null>(null);

	// Computed
	const requiredConsents = $derived(consentTypes.filter((ct) => ct.is_required));
	const optionalConsents = $derived(consentTypes.filter((ct) => !ct.is_required));
	const allRequiredSelected = $derived(
		requiredConsents.every((ct) => selectedConsents.has(ct.code))
	);
	const canSubmit = $derived(allRequiredSelected && (!isMinor || parentGuardianName.trim()));

	// Load consent types
	onMount(async () => {
		try {
			loading = true;
			error = null;
			consentTypes = await consentApi.getConsentTypes(userType);

			// Auto-select all required consents
			requiredConsents.forEach((ct) => {
				selectedConsents.add(ct.code);
			});
		} catch (err) {
			error = err instanceof Error ? err.message : 'เกิดข้อผิดพลาดในการโหลดข้อมูล';
		} finally {
			loading = false;
		}
	});

	// Toggle consent
	function toggleConsent(code: string, isRequired: boolean) {
		if (isRequired) return; // Cannot uncheck required

		if (selectedConsents.has(code)) {
			selectedConsents.delete(code);
		} else {
			selectedConsents.add(code);
		}
	}

	// Submit consents
	async function handleSubmit() {
		try {
			submitting = true;
			error = null;

			const consentsToSubmit: CreateConsentRequest[] = Array.from(selectedConsents).map(
				(code) => ({
					consent_type: code,
					consent_status: 'granted',
					is_minor_consent: isMinor,
					parent_guardian_name: isMinor ? parentGuardianName : undefined,
					parent_relationship: isMinor ? parentRelationship : undefined
				})
			);

			await consentApi.giveMultipleConsents(consentsToSubmit);

			// Success
			if (onComplete) {
				onComplete();
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'ไม่สามารถบันทึกความยินยอมได้';
		} finally {
			submitting = false;
		}
	}

	function handleSkip() {
		if (onSkip) {
			onSkip();
		}
	}
</script>

<div class="mx-auto max-w-3xl space-y-6">
	<!-- Header -->
	<div class="text-center space-y-2">
		<h2 class="text-2xl font-bold">ความยินยอมในการเก็บข้อมูลส่วนบุคคล</h2>
		<p class="text-muted-foreground">ตาม พ.ร.บ. คุ้มครองข้อมูลส่วนบุคคล พ.ศ. 2562 (PDPA)</p>
	</div>

	{#if loading}
		<Card.Root>
			<Card.Content class="flex items-center justify-center p-12">
				<LoaderCircle class="h-8 w-8 animate-spin text-muted-foreground" />
			</Card.Content>
		</Card.Root>
	{:else if error}
		<Card.Root class="border-destructive">
			<Card.Content class="flex items-center gap-3 p-6 text-destructive">
				<AlertCircle class="h-5 w-5" />
				<p>{error}</p>
			</Card.Content>
		</Card.Root>
	{:else}
		<!-- Required Consents -->
		{#if requiredConsents.length > 0}
			<Card.Root>
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<span class="text-destructive">*</span>
						ความยินยอมที่จำเป็น
					</Card.Title>
					<Card.Description>ท่านจำเป็นต้องให้ความยินยอมเหล่านี้เพื่อใช้บริการ</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-4">
					{#each requiredConsents as consent}
						<div class="rounded-lg border p-4 bg-muted/30">
							<div class="flex items-start gap-3">
								<Checkbox
									id={consent.code}
									checked={selectedConsents.has(consent.code)}
									disabled={true}
									class="mt-1"
								/>
								<div class="flex-1 space-y-2">
									<Label for={consent.code} class="text-base font-medium cursor-default">
										{consent.name}
										<span class="text-destructive">*</span>
									</Label>
									<p class="text-sm text-muted-foreground">
										{consent.consent_text_template}
									</p>
									{#if consent.default_duration_days}
										<p class="text-xs text-muted-foreground">
											ความยินยอมนี้มีอายุ {consent.default_duration_days} วัน
										</p>
									{/if}
								</div>
							</div>
						</div>
					{/each}
				</Card.Content>
			</Card.Root>
		{/if}

		<!-- Optional Consents -->
		{#if optionalConsents.length > 0}
			<Card.Root>
				<Card.Header>
					<Card.Title>ความยินยอมเพิ่มเติม (ไม่บังคับ)</Card.Title>
					<Card.Description>ท่านสามารถเลือกให้หรือไม่ให้ความยินยอมเหล่านี้ได้</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-4">
					{#each optionalConsents as consent}
						<div class="rounded-lg border p-4 hover:bg-muted/30 transition-colors">
							<div class="flex items-start gap-3">
								<Checkbox
									id={consent.code}
									checked={selectedConsents.has(consent.code)}
									onCheckedChange={() => toggleConsent(consent.code, consent.is_required)}
									class="mt-1"
								/>
								<div class="flex-1 space-y-2">
									<Label for={consent.code} class="text-base font-medium cursor-pointer">
										{consent.name}
									</Label>
									<p class="text-sm text-muted-foreground">
										{consent.consent_text_template}
									</p>
									{#if consent.default_duration_days}
										<p class="text-xs text-muted-foreground">
											ความยินยอมนี้มีอายุ {consent.default_duration_days} วัน
										</p>
									{/if}
								</div>
							</div>
						</div>
					{/each}
				</Card.Content>
			</Card.Root>
		{/if}

		<!-- Minor Consent (Parent/Guardian Info) -->
		{#if isMinor}
			<Card.Root class="border-primary">
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<span class="text-destructive">*</span>
						ข้อมูลผู้ปกครอง
					</Card.Title>
					<Card.Description>
						สำหรับนักเรียนที่อายุต่ำกว่า 20 ปี ต้องมีผู้ปกครองให้ความยินยอม
					</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-4">
					<div class="space-y-2">
						<Label for="parent-name">
							ชื่อ-นามสกุล ผู้ปกครอง
							<span class="text-destructive">*</span>
						</Label>
						<input
							id="parent-name"
							type="text"
							bind:value={parentGuardianName}
							placeholder="นาย/นาง/นางสาว..."
							class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
							required
						/>
					</div>

					<div class="space-y-2">
						<Label for="parent-relationship">ความสัมพันธ์</Label>
						<select
							id="parent-relationship"
							bind:value={parentRelationship}
							class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
						>
							<option value="father">บิดา</option>
							<option value="mother">มารดา</option>
							<option value="guardian">ผู้ปกครอง</option>
						</select>
					</div>
				</Card.Content>
			</Card.Root>
		{/if}

		<!-- Privacy Policy Link -->
		<Card.Root>
			<Card.Content class="p-4">
				<p class="text-sm text-muted-foreground text-center">
					การให้ความยินยอมนี้ถือว่าท่านได้อ่านและเข้าใจ
					<a
						href="/privacy-policy"
						target="_blank"
						class="text-primary underline hover:no-underline"
					>
						นโยบายความเป็นส่วนตัว
					</a>
					ของเราแล้ว
				</p>
			</Card.Content>
		</Card.Root>

		<!-- Actions -->
		<div class="flex items-center justify-between gap-4">
			{#if showSkipButton}
				<Button variant="ghost" onclick={handleSkip}>ข้ามไปก่อน</Button>
			{:else}
				<div></div>
			{/if}

			<Button onclick={handleSubmit} disabled={!canSubmit || submitting} class="min-w-32">
				{#if submitting}
					<LoaderCircle class="h-4 w-4 animate-spin mr-2" />
					กำลังบันทึก...
				{:else}
					<CheckCircle2 class="h-4 w-4 mr-2" />
					ยืนยันความยินยอม
				{/if}
			</Button>
		</div>
	{/if}
</div>

<style>
	input,
	select {
		transition: all 0.2s;
	}

	input:focus,
	select:focus {
		outline: none;
		border-color: hsl(var(--primary));
		box-shadow: 0 0 0 2px hsl(var(--primary) / 0.2);
	}
</style>
