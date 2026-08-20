<script lang="ts">
	import {
		searchCertificateCandidateAccounts,
		type CertificateCandidateAccount,
		type CertificateTemplateDetail,
		type CreateAccountCertificateCandidateRequest,
		type RecipientType
	} from '$lib/api/certificates';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Search, UserPlus } from 'lucide-svelte';

	let {
		open,
		campaignId,
		templates,
		busy = false,
		onopenchange,
		oncreate
	}: {
		open: boolean;
		campaignId: string;
		templates: CertificateTemplateDetail[];
		busy?: boolean;
		onopenchange: (open: boolean) => void;
		oncreate: (payload: CreateAccountCertificateCandidateRequest) => Promise<void>;
	} = $props();

	let recipientType = $state<Extract<RecipientType, 'student' | 'staff'>>('student');
	let search = $state('');
	let results = $state.raw<CertificateCandidateAccount[]>([]);
	let selected = $state.raw<CertificateCandidateAccount | null>(null);
	let searching = $state(false);
	let searched = $state(false);
	let error = $state('');
	let activityItem = $state('');
	let awardOrRole = $state('');
	let templateId = $state('');
	let searchGeneration = 0;

	const compatibleTemplates = $derived.by(() => {
		const selectedRecipientType = selected?.recipientType;
		if (!selectedRecipientType) return [];
		return templates.filter(
			(template) =>
				template.isActive && template.allowedRecipientTypes.includes(selectedRecipientType)
		);
	});

	function changeRecipientType(value: string) {
		if (value !== 'student' && value !== 'staff') return;
		searchGeneration += 1;
		recipientType = value;
		results = [];
		selected = null;
		searching = false;
		searched = false;
		error = '';
		templateId = '';
	}

	function changeSearch(value: string) {
		searchGeneration += 1;
		search = value;
		results = [];
		selected = null;
		searching = false;
		searched = false;
		error = '';
	}

	async function runSearch() {
		if (search.trim().length < 2 || searching) return;
		const generation = ++searchGeneration;
		const requestedType = recipientType;
		const requestedSearch = search.trim();
		searching = true;
		error = '';
		selected = null;
		try {
			const loadedResults = await searchCertificateCandidateAccounts(campaignId, {
				recipientType: requestedType,
				search: requestedSearch
			});
			if (
				generation !== searchGeneration ||
				requestedType !== recipientType ||
				requestedSearch !== search.trim()
			)
				return;
			results = loadedResults;
			searched = true;
		} catch (searchError) {
			if (generation !== searchGeneration) return;
			error = searchError instanceof Error ? searchError.message : 'ค้นหาบัญชีไม่สำเร็จ';
			results = [];
		} finally {
			if (generation === searchGeneration) searching = false;
		}
	}

	async function submit() {
		if (!selected || busy) return;
		await oncreate({
			userId: selected.userId,
			activityItem: activityItem.trim() || null,
			awardOrRole: awardOrRole.trim() || null,
			templateId: templateId || null,
			customValues: {}
		});
	}
</script>

<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>เพิ่มจากบัญชี</Dialog.Title>
			<Dialog.Description>
				ค้นหานักเรียนหรือบุคลากรจากบัญชีโรงเรียน
				เพื่อให้เกียรติบัตรไปแสดงในพื้นที่ส่วนตัวโดยอัตโนมัติ
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4">
			<div class="grid gap-3 sm:grid-cols-[150px_1fr_auto]">
				<label class="space-y-1.5">
					<span class="text-sm font-medium">ประเภทบัญชี</span>
					<select
						value={recipientType}
						onchange={(event) => changeRecipientType(event.currentTarget.value)}
						class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
					>
						<option value="student">นักเรียน</option>
						<option value="staff">บุคลากร</option>
					</select>
				</label>
				<label class="space-y-1.5">
					<span class="text-sm font-medium">ชื่อ รหัสนักเรียน หรือชื่อผู้ใช้</span>
					<Input
						value={search}
						maxlength={150}
						placeholder="พิมพ์อย่างน้อย 2 ตัวอักษร"
						oninput={(event) => changeSearch(event.currentTarget.value)}
						onkeydown={(event) => {
							if (event.key === 'Enter') {
								event.preventDefault();
								void runSearch();
							}
						}}
					/>
				</label>
				<div class="flex items-end">
					<LoadingButton
						variant="outline"
						loading={searching}
						disabled={search.trim().length < 2}
						onclick={runSearch}
					>
						<Search class="size-4" /> ค้นหา
					</LoadingButton>
				</div>
			</div>

			{#if error}
				<div
					role="alert"
					class="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800"
				>
					{error}
				</div>
			{:else if searched && results.length === 0}
				<div
					class="rounded-lg border border-dashed px-4 py-6 text-center text-sm text-muted-foreground"
				>
					ไม่พบบัญชีที่ตรงกับคำค้น
				</div>
			{:else if results.length > 0}
				<div class="max-h-52 space-y-2 overflow-y-auto rounded-lg border p-2">
					{#each results as account (account.userId)}
						<button
							type="button"
							class={`w-full rounded-md border px-3 py-2 text-left transition-colors ${
								selected?.userId === account.userId
									? 'border-primary bg-primary/5'
									: 'border-transparent hover:bg-muted'
							}`}
							onclick={() => {
								selected = account;
								templateId = '';
							}}
						>
							<span class="block font-medium">
								{account.title ?? ''}{account.firstName}
								{account.lastName}
							</span>
							<span class="block text-xs text-muted-foreground">
								{account.recipientType === 'student'
									? `รหัสนักเรียน ${account.studentId ?? '-'}`
									: `ชื่อผู้ใช้ ${account.staffUsername ?? '-'}`}
							</span>
						</button>
					{/each}
				</div>
			{/if}

			{#if selected}
				<div class="space-y-4 rounded-lg border bg-muted/20 p-4">
					<div class="grid gap-4 sm:grid-cols-2">
						<label class="space-y-1.5">
							<span class="text-sm font-medium">รายการกิจกรรม</span>
							<Input bind:value={activityItem} maxlength={300} />
						</label>
						<label class="space-y-1.5">
							<span class="text-sm font-medium">รางวัลหรือบทบาท</span>
							<Input bind:value={awardOrRole} maxlength={300} />
						</label>
					</div>
					<label class="block space-y-1.5">
						<span class="text-sm font-medium">แบบเกียรติบัตร</span>
						<select
							bind:value={templateId}
							class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
						>
							<option value="">ยังไม่กำหนด</option>
							{#each compatibleTemplates as template (template.id)}
								<option value={template.id}
									>{template.name}{template.isReady ? '' : ' (ยังไม่พร้อม)'}</option
								>
							{/each}
						</select>
					</label>
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => onopenchange(false)}>ยกเลิก</Button>
			<LoadingButton loading={busy} disabled={!selected} onclick={submit}>
				<UserPlus class="size-4" /> เพิ่มบัญชีที่เลือก
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
