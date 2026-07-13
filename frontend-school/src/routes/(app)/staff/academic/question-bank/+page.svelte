<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		createQuestionBankQuestion,
		deleteQuestionBankQuestion,
		getQuestionBankOptions,
		getQuestionBankQuestion,
		listQuestionBankQuestions,
		updateQuestionBankQuestion,
		type QuestionBankSubjectOption,
		type QuestionDetail,
		type QuestionDifficulty,
		type QuestionStatus,
		type QuestionSummary,
		type QuestionType,
		type RichContent,
		type RichContentBlock,
		type UpsertQuestionRequest
	} from '$lib/api/questionBank';
	import { deleteFile, uploadFile } from '$lib/api/files';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import QuestionContent from '$lib/components/question-bank/QuestionContent.svelte';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Textarea } from '$lib/components/ui/textarea';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		Edit3,
		Eye,
		Image as ImageIcon,
		Loader2,
		Plus,
		RefreshCw,
		Save,
		Search,
		Sigma,
		Trash2,
		Upload,
		X
	} from 'lucide-svelte';

	let { data } = $props();

	type QuestionTypeFilter = QuestionType | 'all';
	type DifficultyFilter = QuestionDifficulty | 'all';
	type StatusFilter = QuestionStatus | 'all';
	type EditorMode = 'view' | 'create' | 'edit';
	type ContentDraft = {
		source: RichContent;
		text: string;
		latex: string;
		imageFileId: string;
		imagePreviewUrl: string;
		imageFile: File | null;
		imageAltText: string;
		imageRemoved: boolean;
	};
	type ChoiceDraft = {
		key: string;
		id?: string | null;
		label: string;
		content: ContentDraft;
		isCorrect: boolean;
		sortOrder: number;
	};
	type QuestionDraft = {
		id?: string;
		subjectId: string;
		questionType: QuestionType;
		difficulty: QuestionDifficulty;
		points: number;
		status: QuestionStatus;
		stem: ContentDraft;
		explanation: ContentDraft;
		rubric: ContentDraft;
		tagsText: string;
		choices: ChoiceDraft[];
	};

	const questionTypeOptions: { value: QuestionType; label: string }[] = [
		{ value: 'single_choice', label: 'ตัวเลือกเดียว' },
		{ value: 'multiple_choice', label: 'หลายตัวเลือก' },
		{ value: 'short_answer', label: 'เขียนตอบสั้น' },
		{ value: 'essay', label: 'อัตนัย' }
	];
	const difficultyOptions: { value: QuestionDifficulty; label: string }[] = [
		{ value: 'easy', label: 'ง่าย' },
		{ value: 'medium', label: 'กลาง' },
		{ value: 'hard', label: 'ยาก' }
	];
	const statusOptions: { value: QuestionStatus; label: string }[] = [
		{ value: 'draft', label: 'ร่าง' },
		{ value: 'ready', label: 'พร้อมใช้' },
		{ value: 'archived', label: 'เก็บถาวร' }
	];
	const emptySummary = { total: 0, choice: 0, written: 0, ready: 0 };
	const pageSize = 20;
	const maxImageBytes = 10 * 1024 * 1024;

	const canReadQuestionBank = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_QUESTION_BANK_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_QUESTION_BANK_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_QUESTION_BANK_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_QUESTION_BANK_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL
		)
	);
	const hasManagePermission = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_QUESTION_BANK_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL
		)
	);

	let choiceKey = 0;
	let loading = $state(true);
	let loadError = $state('');
	let loadingQuestions = $state(false);
	let loadingDetail = $state(false);
	let saving = $state(false);
	let deleting = $state(false);
	let subjects = $state<QuestionBankSubjectOption[]>([]);
	let questions = $state<QuestionSummary[]>([]);
	let summary = $state({ ...emptySummary });
	let currentPage = $state(1);
	let totalPages = $state(1);
	let totalQuestions = $state(0);
	let selectedSubjectId = $state('');
	let selectedQuestionType = $state<QuestionTypeFilter>('all');
	let selectedDifficulty = $state<DifficultyFilter>('all');
	let selectedStatus = $state<StatusFilter>('all');
	let search = $state('');
	let tag = $state('');
	let editorOpen = $state(false);
	let editorMode = $state<EditorMode>('view');
	let detail = $state<QuestionDetail | null>(null);
	let draft = $state<QuestionDraft>(newDraft());
	let deleteTarget = $state<QuestionSummary | null>(null);
	let deleteDialogOpen = $state(false);

	const creatableSubjects = $derived(subjects.filter((subject) => subject.canCreate));
	const canCreateQuestion = $derived(hasManagePermission && creatableSubjects.length > 0);
	const isChoiceQuestion = $derived(
		draft.questionType === 'single_choice' || draft.questionType === 'multiple_choice'
	);

	onMount(() => {
		void loadInitialData();
	});

	onDestroy(() => cleanupDraftObjectUrls(draft));

	function emptyContentDraft(): ContentDraft {
		return {
			source: { blocks: [] },
			text: '',
			latex: '',
			imageFileId: '',
			imagePreviewUrl: '',
			imageFile: null,
			imageAltText: '',
			imageRemoved: false
		};
	}

	function newChoice(label: string, index: number): ChoiceDraft {
		return {
			key: `choice-${++choiceKey}`,
			label,
			content: emptyContentDraft(),
			isCorrect: index === 0,
			sortOrder: index + 1
		};
	}

	function defaultChoices() {
		return ['A', 'B', 'C', 'D'].map((label, index) => newChoice(label, index));
	}

	function newDraft(subjectId = ''): QuestionDraft {
		return {
			subjectId,
			questionType: 'single_choice',
			difficulty: 'medium',
			points: 1,
			status: 'draft',
			stem: emptyContentDraft(),
			explanation: emptyContentDraft(),
			rubric: emptyContentDraft(),
			tagsText: '',
			choices: defaultChoices()
		};
	}

	function firstBlock(content: RichContent | null | undefined, type: RichContentBlock['type']) {
		return content?.blocks.find((block) => block.type === type);
	}

	function textFromContent(content: RichContent | null | undefined) {
		const block = firstBlock(content, 'paragraph');
		return block?.type === 'paragraph' ? block.text : '';
	}

	function latexFromContent(content: RichContent | null | undefined) {
		const block = firstBlock(content, 'math');
		return block?.type === 'math' ? block.latex : '';
	}

	function imageFromContent(content: RichContent | null | undefined) {
		const block = firstBlock(content, 'image');
		return block?.type === 'image' ? block : null;
	}

	function contentDraftFrom(
		content: RichContent | null | undefined,
		fileUrls: Map<string, string>
	): ContentDraft {
		const source = content ?? { blocks: [] };
		const image = imageFromContent(source);
		return {
			source,
			text: textFromContent(source),
			latex: latexFromContent(source),
			imageFileId: image?.fileId ?? '',
			imagePreviewUrl: image ? (fileUrls.get(image.fileId) ?? '') : '',
			imageFile: null,
			imageAltText: image?.altText ?? '',
			imageRemoved: false
		};
	}

	function choiceDraftFromDetail(
		choice: QuestionDetail['choices'][number],
		index: number,
		fileUrls: Map<string, string>
	): ChoiceDraft {
		return {
			key: choice.id,
			id: choice.id,
			label: choice.label,
			content: contentDraftFrom(choice.content, fileUrls),
			isCorrect: choice.isCorrect,
			sortOrder: choice.sortOrder || index + 1
		};
	}

	function draftFromDetail(question: QuestionDetail): QuestionDraft {
		const fileUrls = new Map(
			question.files.map((file) => [file.id, file.thumbnailUrl ?? file.url])
		);
		return {
			id: question.id,
			subjectId: question.subjectId ?? '',
			questionType: question.questionType,
			difficulty: question.difficulty,
			points: question.points,
			status: question.status,
			stem: contentDraftFrom(question.stemContent, fileUrls),
			explanation: contentDraftFrom(question.explanationContent, fileUrls),
			rubric: contentDraftFrom(question.rubricContent, fileUrls),
			tagsText: question.tags.join(', '),
			choices: question.choices.map((choice, index) =>
				choiceDraftFromDetail(choice, index, fileUrls)
			)
		};
	}

	function mergeContent(content: ContentDraft): RichContent {
		const blocks: RichContentBlock[] = [];
		let paragraphHandled = false;
		let mathHandled = false;
		let imageHandled = false;

		for (const block of content.source.blocks) {
			if (block.type === 'paragraph' && !paragraphHandled) {
				paragraphHandled = true;
				if (content.text.trim()) blocks.push({ type: 'paragraph', text: content.text.trim() });
				continue;
			}
			if (block.type === 'math' && !mathHandled) {
				mathHandled = true;
				if (content.latex.trim()) {
					blocks.push({ type: 'math', latex: content.latex.trim(), display: block.display });
				}
				continue;
			}
			if (block.type === 'image') {
				if (content.imageRemoved) continue;
				if (!imageHandled) {
					imageHandled = true;
					if (content.imageFileId) {
						blocks.push({
							type: 'image',
							fileId: content.imageFileId,
							altText: content.imageAltText.trim() || null,
							caption: block.caption ?? null
						});
					}
					continue;
				}
			}
			blocks.push(block);
		}

		if (!paragraphHandled && content.text.trim()) {
			blocks.push({ type: 'paragraph', text: content.text.trim() });
		}
		if (!mathHandled && content.latex.trim()) {
			blocks.push({ type: 'math', latex: content.latex.trim(), display: true });
		}
		if (!imageHandled && !content.imageRemoved && content.imageFileId) {
			blocks.push({
				type: 'image',
				fileId: content.imageFileId,
				altText: content.imageAltText.trim() || null,
				caption: null
			});
		}
		return { blocks };
	}

	function tagsFromText(value: string) {
		const tags: string[] = [];
		for (const rawTag of value.split(',')) {
			const normalized = rawTag.trim().toLowerCase();
			if (normalized && !tags.includes(normalized)) tags.push(normalized);
		}
		return tags;
	}

	function questionTitle(question: QuestionSummary) {
		return (
			textFromContent(question.stemContent) ||
			latexFromContent(question.stemContent) ||
			(imageFromContent(question.stemContent) ? 'โจทย์รูปภาพ' : 'โจทย์')
		);
	}

	function subjectLabel(question: QuestionSummary) {
		return (
			[question.subjectCode, question.subjectNameTh || question.subjectNameEn]
				.filter(Boolean)
				.join(' ') || 'ยังไม่ผูกรายวิชา'
		);
	}

	function subjectOptionLabel(subject: QuestionBankSubjectOption) {
		return `${subject.code} ${subject.nameTh}`;
	}

	function typeLabel(value: QuestionType) {
		return questionTypeOptions.find((option) => option.value === value)?.label ?? value;
	}

	function difficultyLabel(value: QuestionDifficulty) {
		return difficultyOptions.find((option) => option.value === value)?.label ?? value;
	}

	function statusLabel(value: QuestionStatus) {
		return statusOptions.find((option) => option.value === value)?.label ?? value;
	}

	function statusVariant(value: QuestionStatus) {
		if (value === 'ready') return 'default';
		if (value === 'archived') return 'secondary';
		return 'outline';
	}

	async function loadInitialData() {
		if (!canReadQuestionBank) {
			loading = false;
			return;
		}
		loading = true;
		loadError = '';
		try {
			const [optionsResponse, pageResponse] = await Promise.all([
				getQuestionBankOptions(),
				listQuestionBankQuestions({ page: 1, pageSize })
			]);
			subjects = optionsResponse.subjects;
			applyPage(pageResponse);
		} catch (error) {
			loadError = error instanceof Error ? error.message : 'โหลดคลังข้อสอบไม่สำเร็จ';
		} finally {
			loading = false;
		}
	}

	function applyPage(pageResponse: Awaited<ReturnType<typeof listQuestionBankQuestions>>) {
		questions = pageResponse.items;
		summary = pageResponse.summary;
		currentPage = pageResponse.page;
		totalPages = pageResponse.totalPages;
		totalQuestions = pageResponse.total;
	}

	async function loadQuestions(page = 1) {
		loadingQuestions = true;
		loadError = '';
		try {
			const pageResponse = await listQuestionBankQuestions({
				subjectId: selectedSubjectId,
				questionType: selectedQuestionType,
				difficulty: selectedDifficulty,
				status: selectedStatus,
				search,
				tag,
				page,
				pageSize
			});
			applyPage(pageResponse);
		} catch (error) {
			loadError = error instanceof Error ? error.message : 'โหลดคลังข้อสอบไม่สำเร็จ';
			toast.error(loadError);
		} finally {
			loadingQuestions = false;
		}
	}

	function startCreate() {
		const preferredSubject = creatableSubjects.find((subject) => subject.id === selectedSubjectId);
		const subject = preferredSubject ?? creatableSubjects[0];
		if (!subject) {
			toast.error('ยังไม่มีรายวิชาที่คุณมีสิทธิ์เพิ่มข้อสอบ');
			return;
		}
		cleanupDraftObjectUrls(draft);
		detail = null;
		draft = newDraft(subject.id);
		editorMode = 'create';
		editorOpen = true;
	}

	async function openQuestion(question: QuestionSummary, mode: 'view' | 'edit') {
		if (mode === 'edit' && !question.canManage) return;
		cleanupDraftObjectUrls(draft);
		editorMode = mode;
		detail = null;
		editorOpen = true;
		loadingDetail = true;
		try {
			const response = await getQuestionBankQuestion(question.id);
			detail = response;
			if (mode === 'edit') draft = draftFromDetail(response);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'โหลดข้อสอบไม่สำเร็จ');
			editorOpen = false;
		} finally {
			loadingDetail = false;
		}
	}

	function handleDialogOpenChange(open: boolean) {
		editorOpen = open;
		if (!open) {
			cleanupDraftObjectUrls(draft);
			detail = null;
		}
	}

	function handleQuestionTypeChange(value: QuestionType) {
		draft.questionType = value;
		if (value === 'single_choice' || value === 'multiple_choice') {
			if (draft.choices.length < 2) draft.choices = defaultChoices();
			if (
				value === 'single_choice' &&
				draft.choices.filter((choice) => choice.isCorrect).length !== 1
			) {
				draft.choices = draft.choices.map((choice, index) => ({
					...choice,
					isCorrect: index === 0
				}));
			}
		}
	}

	function toggleCorrectChoice(index: number) {
		if (draft.questionType === 'single_choice') {
			draft.choices = draft.choices.map((choice, choiceIndex) => ({
				...choice,
				isCorrect: choiceIndex === index
			}));
			return;
		}
		draft.choices[index].isCorrect = !draft.choices[index].isCorrect;
	}

	function addChoice() {
		const label = String.fromCharCode(65 + draft.choices.length);
		draft.choices = [...draft.choices, newChoice(label, draft.choices.length)];
	}

	function removeChoice(index: number) {
		if (draft.choices.length <= 2) return;
		revokeContentObjectUrl(draft.choices[index].content);
		draft.choices = draft.choices
			.filter((_, choiceIndex) => choiceIndex !== index)
			.map((choice, choiceIndex) => ({ ...choice, sortOrder: choiceIndex + 1 }));
		if (
			draft.questionType === 'single_choice' &&
			draft.choices.filter((choice) => choice.isCorrect).length !== 1
		) {
			draft.choices = draft.choices.map((choice, choiceIndex) => ({
				...choice,
				isCorrect: choiceIndex === 0
			}));
		}
	}

	function selectDraftImage(event: Event, target: ContentDraft) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;
		if (!file.type.startsWith('image/')) {
			toast.error('กรุณาเลือกไฟล์รูปภาพ');
			return;
		}
		if (file.size > maxImageBytes) {
			toast.error('รูปต้องมีขนาดไม่เกิน 10 MB');
			return;
		}

		revokeContentObjectUrl(target);
		target.imageFile = file;
		target.imageFileId = '';
		target.imagePreviewUrl = URL.createObjectURL(file);
		target.imageRemoved = false;
	}

	function removeDraftImage(target: ContentDraft) {
		revokeContentObjectUrl(target);
		target.imageFile = null;
		target.imageFileId = '';
		target.imagePreviewUrl = '';
		target.imageRemoved = true;
	}

	function revokeContentObjectUrl(content: ContentDraft) {
		if (content.imageFile && content.imagePreviewUrl.startsWith('blob:')) {
			URL.revokeObjectURL(content.imagePreviewUrl);
		}
	}

	function cleanupDraftObjectUrls(value: QuestionDraft) {
		revokeContentObjectUrl(value.stem);
		revokeContentObjectUrl(value.explanation);
		revokeContentObjectUrl(value.rubric);
		for (const choice of value.choices) revokeContentObjectUrl(choice.content);
	}

	function validateDraft() {
		if (!draft.subjectId) return 'กรุณาเลือกรายวิชา';
		if (!contentDraftHasBody(draft.stem)) return 'กรุณาระบุโจทย์';
		if (!Number.isFinite(Number(draft.points)) || Number(draft.points) < 0) {
			return 'คะแนนต้องเป็นตัวเลขที่ไม่ติดลบ';
		}
		if (isChoiceQuestion) {
			if (draft.choices.length < 2) return 'ข้อสอบแบบตัวเลือกต้องมีอย่างน้อย 2 ตัวเลือก';
			if (
				draft.choices.some((choice) => !choice.label.trim() || !contentDraftHasBody(choice.content))
			) {
				return 'กรุณาระบุป้ายและเนื้อหาของตัวเลือกให้ครบ';
			}
			const correctCount = draft.choices.filter((choice) => choice.isCorrect).length;
			if (draft.questionType === 'single_choice' && correctCount !== 1) {
				return 'ตัวเลือกเดียวต้องมีคำตอบถูก 1 ข้อ';
			}
			if (draft.questionType === 'multiple_choice' && correctCount < 1) {
				return 'หลายตัวเลือกต้องมีคำตอบถูกอย่างน้อย 1 ข้อ';
			}
		}
		return '';
	}

	function contentDraftHasBody(content: ContentDraft) {
		return Boolean(
			content.text.trim() || content.latex.trim() || content.imageFile || content.imageFileId
		);
	}

	function pendingImageContents() {
		const contents = [draft.stem, draft.explanation, draft.rubric];
		for (const choice of draft.choices) contents.push(choice.content);
		return contents.filter((content) => content.imageFile);
	}

	function buildPayload(): UpsertQuestionRequest {
		const explanationContent = mergeContent(draft.explanation);
		const rubricContent = mergeContent(draft.rubric);
		return {
			subjectId: draft.subjectId,
			questionType: draft.questionType,
			difficulty: draft.difficulty,
			points: Number(draft.points),
			stemContent: mergeContent(draft.stem),
			explanationContent: explanationContent.blocks.length ? explanationContent : null,
			rubricContent: rubricContent.blocks.length ? rubricContent : null,
			tags: tagsFromText(draft.tagsText),
			status: draft.status,
			choices: isChoiceQuestion
				? draft.choices.map((choice, index) => ({
						id: choice.id ?? null,
						label: choice.label.trim(),
						content: mergeContent(choice.content),
						isCorrect: choice.isCorrect,
						sortOrder: index + 1
					}))
				: []
		};
	}

	async function saveQuestion() {
		const validationError = validateDraft();
		if (validationError) {
			toast.error(validationError);
			return;
		}

		saving = true;
		const uploadedIds: string[] = [];
		let saveRequestStarted = false;
		try {
			for (const content of pendingImageContents()) {
				if (!content.imageFile) continue;
				const response = await uploadFile(content.imageFile, 'course_material', true);
				content.imageFileId = response.file.id;
				uploadedIds.push(response.file.id);
			}
			const payload = buildPayload();
			saveRequestStarted = true;
			if (draft.id) {
				await updateQuestionBankQuestion(draft.id, payload);
			} else {
				await createQuestionBankQuestion(payload);
			}

			cleanupDraftObjectUrls(draft);
			editorOpen = false;
			detail = null;
			await loadQuestions(draft.id ? currentPage : 1);
			toast.success('บันทึกข้อสอบแล้ว');
		} catch (error) {
			for (const content of pendingImageContents()) content.imageFileId = '';
			if (!saveRequestStarted) {
				const cleanupResults = await Promise.allSettled(uploadedIds.map((id) => deleteFile(id)));
				if (cleanupResults.some((result) => result.status === 'rejected')) {
					toast.warning('มีไฟล์ชั่วคราวบางส่วนรอระบบเก็บกวาดอัตโนมัติ');
				}
			} else if (uploadedIds.length) {
				toast.warning('ไฟล์ที่ยังไม่ถูกใช้งานจะถูกเก็บกวาดอัตโนมัติภายใน 24 ชั่วโมง');
			}
			toast.error(error instanceof Error ? error.message : 'บันทึกข้อสอบไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	function requestDelete(question: QuestionSummary) {
		if (!question.canManage) return;
		deleteTarget = question;
		deleteDialogOpen = true;
	}

	async function confirmDelete() {
		if (!deleteTarget?.canManage) return;
		deleting = true;
		try {
			await deleteQuestionBankQuestion(deleteTarget.id);
			deleteDialogOpen = false;
			deleteTarget = null;
			await loadQuestions(
				questions.length === 1 && currentPage > 1 ? currentPage - 1 : currentPage
			);
			toast.success('ลบข้อสอบแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ลบข้อสอบไม่สำเร็จ');
		} finally {
			deleting = false;
		}
	}
</script>

<PageShell title={data.title} description="คลังกลางสำหรับเก็บและค้นหาข้อสอบตามรายวิชา">
	{#snippet actions()}
		<div class="flex items-center gap-2">
			<Button
				variant="outline"
				onclick={() => loadQuestions(currentPage)}
				disabled={loadingQuestions}
			>
				{#if loadingQuestions}
					<Loader2 class="h-4 w-4 animate-spin" />
				{:else}
					<RefreshCw class="h-4 w-4" />
				{/if}
				รีเฟรช
			</Button>
			{#if canCreateQuestion}
				<Button onclick={startCreate}>
					<Plus class="h-4 w-4" />
					เพิ่มข้อสอบ
				</Button>
			{/if}
		</div>
	{/snippet}

	{#if loading}
		<PageSkeleton />
	{:else if !canReadQuestionBank}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์เข้าคลังข้อสอบ"
			description="ติดต่อผู้ดูแลระบบเพื่อขอสิทธิ์คลังข้อสอบ"
		/>
	{:else if loadError && questions.length === 0}
		<PageState
			variant="error"
			title="โหลดคลังข้อสอบไม่สำเร็จ"
			description={loadError}
			actionLabel="ลองอีกครั้ง"
			onaction={loadInitialData}
		/>
	{:else}
		<section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
			{#each [['ทั้งหมด', summary.total], ['ตัวเลือก', summary.choice], ['เขียนตอบ', summary.written], ['พร้อมใช้', summary.ready]] as item (item[0])}
				<div class="rounded-lg border bg-card p-4">
					<p class="text-sm text-muted-foreground">{item[0]}</p>
					<p class="mt-1 text-2xl font-semibold">{item[1]}</p>
				</div>
			{/each}
		</section>

		<section class="space-y-4">
			<div class="rounded-lg border bg-card p-4">
				<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-6">
					<div class="md:col-span-2 xl:col-span-2">
						<Label for="question-search">ค้นหา</Label>
						<div class="relative mt-1">
							<Search class="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
							<Input
								id="question-search"
								class="pl-9"
								placeholder="ข้อความโจทย์หรือรหัสวิชา"
								bind:value={search}
								onkeydown={(event) => event.key === 'Enter' && void loadQuestions(1)}
							/>
						</div>
					</div>
					<div>
						<Label>รายวิชา</Label>
						<Select.Root type="single" bind:value={selectedSubjectId}>
							<Select.Trigger class="mt-1 w-full">
								{subjects.find((subject) => subject.id === selectedSubjectId)
									? subjectOptionLabel(
											subjects.find((subject) => subject.id === selectedSubjectId)!
										)
									: 'ทุกวิชา'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="">ทุกวิชา</Select.Item>
								{#each subjects as subject (subject.id)}
									<Select.Item value={subject.id}>{subjectOptionLabel(subject)}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div>
						<Label>ประเภท</Label>
						<Select.Root type="single" bind:value={selectedQuestionType}>
							<Select.Trigger class="mt-1 w-full">
								{selectedQuestionType === 'all' ? 'ทุกประเภท' : typeLabel(selectedQuestionType)}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="all">ทุกประเภท</Select.Item>
								{#each questionTypeOptions as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div>
						<Label>สถานะ</Label>
						<Select.Root type="single" bind:value={selectedStatus}>
							<Select.Trigger class="mt-1 w-full">
								{selectedStatus === 'all' ? 'ทุกสถานะ' : statusLabel(selectedStatus)}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="all">ทุกสถานะ</Select.Item>
								{#each statusOptions as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div>
						<Label>ความยาก</Label>
						<Select.Root type="single" bind:value={selectedDifficulty}>
							<Select.Trigger class="mt-1 w-full">
								{selectedDifficulty === 'all' ? 'ทุกระดับ' : difficultyLabel(selectedDifficulty)}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="all">ทุกระดับ</Select.Item>
								{#each difficultyOptions as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="md:col-span-2 xl:col-span-2">
						<Label for="question-tag">Tag</Label>
						<Input
							id="question-tag"
							class="mt-1"
							placeholder="เช่น สมการ, กลางภาค"
							bind:value={tag}
						/>
					</div>
					<div class="flex items-end xl:col-start-6">
						<LoadingButton
							class="w-full"
							loading={loadingQuestions}
							onclick={() => loadQuestions(1)}
						>
							<Search class="h-4 w-4" />
							ค้นหา
						</LoadingButton>
					</div>
				</div>
			</div>

			{#if loadError}
				<PageState
					variant="error"
					title="โหลดรายการล่าสุดไม่สำเร็จ"
					description={loadError}
					actionLabel="ลองอีกครั้ง"
					onaction={() => loadQuestions(currentPage)}
				/>
			{/if}

			{#if questions.length === 0}
				<PageState
					title="ยังไม่พบข้อสอบ"
					description="เพิ่มข้อสอบใหม่ หรือเปลี่ยนตัวกรองเพื่อค้นหา"
					actionLabel={canCreateQuestion ? 'เพิ่มข้อสอบ' : undefined}
					onaction={canCreateQuestion ? startCreate : undefined}
				/>
			{:else}
				<div class="overflow-hidden rounded-lg border bg-card">
					{#each questions as question (question.id)}
						<article class="border-b p-4 last:border-b-0">
							<div class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
								<div class="min-w-0 space-y-2">
									<div class="flex flex-wrap items-center gap-2">
										<Badge variant={statusVariant(question.status)}
											>{statusLabel(question.status)}</Badge
										>
										<Badge variant="outline">{typeLabel(question.questionType)}</Badge>
										<Badge variant="secondary">{difficultyLabel(question.difficulty)}</Badge>
										{#if imageFromContent(question.stemContent)}
											<Badge variant="outline"><ImageIcon class="h-3 w-3" /> รูป</Badge>
										{/if}
										{#if latexFromContent(question.stemContent)}
											<Badge variant="outline"><Sigma class="h-3 w-3" /> สูตร</Badge>
										{/if}
									</div>
									<h2 class="line-clamp-2 text-base font-medium">{questionTitle(question)}</h2>
									<div class="flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
										<span>{subjectLabel(question)}</span>
										<span>{question.points} คะแนน</span>
										{#if question.choiceCount}<span>{question.choiceCount} ตัวเลือก</span>{/if}
									</div>
									{#if question.tags.length}
										<div class="flex flex-wrap gap-1">
											{#each question.tags as item (item)}<Badge variant="outline">{item}</Badge
												>{/each}
										</div>
									{/if}
								</div>
								<div class="flex shrink-0 gap-2">
									<Button
										variant="outline"
										size="sm"
										onclick={() => openQuestion(question, 'view')}
									>
										<Eye class="h-4 w-4" /> ดู
									</Button>
									{#if question.canManage}
										<Button
											variant="outline"
											size="sm"
											onclick={() => openQuestion(question, 'edit')}
										>
											<Edit3 class="h-4 w-4" /> แก้ไข
										</Button>
										<Button variant="destructive" size="sm" onclick={() => requestDelete(question)}>
											<Trash2 class="h-4 w-4" /> ลบ
										</Button>
									{/if}
								</div>
							</div>
						</article>
					{/each}
				</div>

				<div class="flex flex-col items-center justify-between gap-3 sm:flex-row">
					<p class="text-sm text-muted-foreground">
						ทั้งหมด {totalQuestions} ข้อ · หน้า {currentPage} จาก {totalPages}
					</p>
					<div class="flex gap-2">
						<Button
							variant="outline"
							disabled={currentPage <= 1 || loadingQuestions}
							onclick={() => loadQuestions(currentPage - 1)}>ก่อนหน้า</Button
						>
						<Button
							variant="outline"
							disabled={currentPage >= totalPages || loadingQuestions}
							onclick={() => loadQuestions(currentPage + 1)}>ถัดไป</Button
						>
					</div>
				</div>
			{/if}
		</section>
	{/if}
</PageShell>

<Dialog.Root bind:open={editorOpen} onOpenChange={handleDialogOpenChange}>
	<Dialog.Content class="max-h-[92vh] overflow-y-auto sm:max-w-4xl">
		<Dialog.Header>
			<Dialog.Title>
				{editorMode === 'view'
					? 'รายละเอียดข้อสอบ'
					: editorMode === 'edit'
						? 'แก้ไขข้อสอบ'
						: 'เพิ่มข้อสอบ'}
			</Dialog.Title>
			<Dialog.Description>
				{editorMode === 'view'
					? 'ดูโจทย์ ตัวเลือก และเฉลยที่เก็บไว้ในคลัง'
					: 'ข้อสอบจะผูกกับรายวิชาที่เลือกโดยตรง'}
			</Dialog.Description>
		</Dialog.Header>

		{#if loadingDetail}
			<div class="flex min-h-64 items-center justify-center">
				<Loader2 class="h-8 w-8 animate-spin" />
			</div>
		{:else if editorMode === 'view' && detail}
			<div class="space-y-5">
				<div class="flex flex-wrap gap-2">
					<Badge variant={statusVariant(detail.status)}>{statusLabel(detail.status)}</Badge>
					<Badge variant="outline">{typeLabel(detail.questionType)}</Badge>
					<Badge variant="secondary">{difficultyLabel(detail.difficulty)}</Badge>
					<Badge variant="outline">{subjectLabel(detail)}</Badge>
				</div>
				<section class="rounded-lg border p-4">
					<h3 class="mb-3 font-medium">โจทย์</h3>
					<QuestionContent content={detail.stemContent} files={detail.files} />
				</section>
				{#if detail.choices.length}
					<section class="space-y-2">
						<h3 class="font-medium">ตัวเลือก</h3>
						{#each detail.choices as choice (choice.id)}
							<div class="flex gap-3 rounded-lg border p-3" class:border-primary={choice.isCorrect}>
								<Badge variant={choice.isCorrect ? 'default' : 'outline'}>{choice.label}</Badge>
								<div class="min-w-0 flex-1">
									<QuestionContent content={choice.content} files={detail.files} />
								</div>
							</div>
						{/each}
					</section>
				{/if}
				{#if detail.explanationContent?.blocks.length}
					<section class="rounded-lg border p-4">
						<h3 class="mb-2 font-medium">เฉลย/คำอธิบาย</h3>
						<QuestionContent content={detail.explanationContent} files={detail.files} />
					</section>
				{/if}
				{#if detail.rubricContent?.blocks.length}
					<section class="rounded-lg border p-4">
						<h3 class="mb-2 font-medium">เกณฑ์ให้คะแนน</h3>
						<QuestionContent content={detail.rubricContent} files={detail.files} />
					</section>
				{/if}
				{#if detail.tags.length}
					<div class="flex flex-wrap gap-1">
						{#each detail.tags as item (item)}<Badge variant="outline">{item}</Badge>{/each}
					</div>
				{/if}
				{#if detail.canManage}
					<Dialog.Footer>
						<Button onclick={() => openQuestion(detail!, 'edit')}
							><Edit3 class="h-4 w-4" /> แก้ไข</Button
						>
					</Dialog.Footer>
				{/if}
			</div>
		{:else if editorMode !== 'view'}
			<div class="space-y-5">
				<div class="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
					<div class="md:col-span-2 lg:col-span-3">
						<Label>รายวิชา <span class="text-destructive">*</span></Label>
						<Select.Root type="single" bind:value={draft.subjectId}>
							<Select.Trigger class="mt-1 w-full">
								{subjects.find((subject) => subject.id === draft.subjectId)
									? subjectOptionLabel(subjects.find((subject) => subject.id === draft.subjectId)!)
									: 'เลือกรายวิชา'}
							</Select.Trigger>
							<Select.Content>
								{#each subjects as subject (subject.id)}
									<Select.Item
										value={subject.id}
										disabled={!subject.canCreate && subject.id !== draft.subjectId}
									>
										{subjectOptionLabel(subject)}
									</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div>
						<Label>ประเภทข้อ</Label>
						<Select.Root
							type="single"
							value={draft.questionType}
							onValueChange={(value) => handleQuestionTypeChange(value as QuestionType)}
						>
							<Select.Trigger class="mt-1 w-full">{typeLabel(draft.questionType)}</Select.Trigger>
							<Select.Content>
								{#each questionTypeOptions as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div>
						<Label>ความยาก</Label>
						<Select.Root type="single" bind:value={draft.difficulty}>
							<Select.Trigger class="mt-1 w-full"
								>{difficultyLabel(draft.difficulty)}</Select.Trigger
							>
							<Select.Content>
								{#each difficultyOptions as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div>
						<Label>สถานะ</Label>
						<Select.Root type="single" bind:value={draft.status}>
							<Select.Trigger class="mt-1 w-full">{statusLabel(draft.status)}</Select.Trigger>
							<Select.Content>
								{#each statusOptions as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div>
						<Label for="question-points">คะแนน</Label>
						<Input
							id="question-points"
							class="mt-1"
							type="number"
							min="0"
							step="0.5"
							bind:value={draft.points}
						/>
					</div>
				</div>

				<div class="space-y-3 rounded-lg border p-4">
					<Label for="stem-text">โจทย์ <span class="text-destructive">*</span></Label>
					<Textarea id="stem-text" class="min-h-28" bind:value={draft.stem.text} />
					<div>
						<Label for="stem-latex">สมการ LaTeX</Label>
						<Input id="stem-latex" class="mt-1 font-mono" bind:value={draft.stem.latex} />
					</div>
					<div>
						<Label for="stem-image">รูปประกอบ</Label>
						<div class="mt-1 flex flex-wrap items-center gap-2">
							<Input
								id="stem-image"
								class="max-w-sm"
								type="file"
								accept="image/*"
								onchange={(event) => selectDraftImage(event, draft.stem)}
							/>
							<Upload class="h-4 w-4 text-muted-foreground" />
							<span class="text-xs text-muted-foreground">จะอัปโหลดเมื่อกดบันทึกเท่านั้น</span>
						</div>
					</div>
					{#if draft.stem.imagePreviewUrl}
						<div class="space-y-2">
							<img
								src={draft.stem.imagePreviewUrl}
								alt={draft.stem.imageAltText}
								class="max-h-64 rounded-md border object-contain"
							/>
							<div class="flex gap-2">
								<Input
									placeholder="คำอธิบายรูปสำหรับผู้ใช้โปรแกรมอ่านจอ"
									bind:value={draft.stem.imageAltText}
								/>
								<Button
									variant="outline"
									size="icon"
									aria-label="นำรูปโจทย์ออก"
									onclick={() => removeDraftImage(draft.stem)}
								>
									<X class="h-4 w-4" />
								</Button>
							</div>
						</div>
					{/if}
				</div>

				{#if isChoiceQuestion}
					<div class="space-y-3">
						<div class="flex items-center justify-between">
							<Label>ตัวเลือก</Label>
							<Button variant="outline" size="sm" onclick={addChoice}
								><Plus class="h-4 w-4" /> เพิ่มตัวเลือก</Button
							>
						</div>
						{#each draft.choices as choice, index (choice.key)}
							<div class="space-y-3 rounded-lg border p-3">
								<div class="flex items-center gap-2">
									<Input
										aria-label={`ป้ายตัวเลือก ${index + 1}`}
										class="h-9 w-16"
										bind:value={choice.label}
									/>
									<Button
										variant={choice.isCorrect ? 'default' : 'outline'}
										size="sm"
										onclick={() => toggleCorrectChoice(index)}
									>
										{choice.isCorrect ? 'เป็นเฉลย' : 'เลือกเป็นเฉลย'}
									</Button>
									<Button
										class="ml-auto"
										variant="ghost"
										size="icon"
										aria-label={`ลบตัวเลือก ${choice.label}`}
										disabled={draft.choices.length <= 2}
										onclick={() => removeChoice(index)}
									>
										<Trash2 class="h-4 w-4" />
									</Button>
								</div>
								<Textarea
									aria-label={`เนื้อหาตัวเลือก ${choice.label}`}
									class="min-h-20"
									bind:value={choice.content.text}
								/>
								<Input
									aria-label={`สมการตัวเลือก ${choice.label}`}
									class="font-mono"
									placeholder="LaTeX (ถ้ามี)"
									bind:value={choice.content.latex}
								/>
								<div class="flex flex-wrap items-center gap-2">
									<Input
										class="max-w-sm"
										aria-label={`รูปตัวเลือก ${choice.label}`}
										type="file"
										accept="image/*"
										onchange={(event) => selectDraftImage(event, choice.content)}
									/>
									<span class="text-xs text-muted-foreground">อัปโหลดเมื่อกดบันทึก</span>
								</div>
								{#if choice.content.imagePreviewUrl}
									<div class="space-y-2">
										<img
											src={choice.content.imagePreviewUrl}
											alt={choice.content.imageAltText}
											class="max-h-40 rounded-md border object-contain"
										/>
										<div class="flex gap-2">
											<Input placeholder="คำอธิบายรูป" bind:value={choice.content.imageAltText} />
											<Button
												variant="outline"
												size="icon"
												aria-label={`นำรูปตัวเลือก ${choice.label} ออก`}
												onclick={() => removeDraftImage(choice.content)}
												><X class="h-4 w-4" /></Button
											>
										</div>
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{/if}

				<div class="grid gap-3 md:grid-cols-2">
					<div>
						<Label for="explanation-text">เฉลย/คำอธิบาย</Label>
						<Textarea
							id="explanation-text"
							class="mt-1 min-h-24"
							bind:value={draft.explanation.text}
						/>
					</div>
					<div>
						<Label for="rubric-text">เกณฑ์ให้คะแนน</Label>
						<Textarea id="rubric-text" class="mt-1 min-h-24" bind:value={draft.rubric.text} />
					</div>
				</div>
				<div>
					<Label for="tags-text">Tags</Label>
					<Input
						id="tags-text"
						class="mt-1"
						placeholder="คั่นด้วยเครื่องหมายจุลภาค"
						bind:value={draft.tagsText}
					/>
				</div>

				<Dialog.Footer>
					<Button variant="outline" onclick={() => handleDialogOpenChange(false)} disabled={saving}
						>ยกเลิก</Button
					>
					<LoadingButton loading={saving} onclick={saveQuestion}>
						<Save class="h-4 w-4" /> บันทึกข้อสอบ
					</LoadingButton>
				</Dialog.Footer>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>

<AlertDialog.Root bind:open={deleteDialogOpen}>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>ยืนยันการลบข้อสอบ</AlertDialog.Title>
			<AlertDialog.Description>
				ลบ “{deleteTarget ? questionTitle(deleteTarget) : ''}” ออกจากคลังข้อสอบหรือไม่?
				การดำเนินการนี้ย้อนกลับไม่ได้
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel disabled={deleting}>ยกเลิก</AlertDialog.Cancel>
			<AlertDialog.Action disabled={deleting} onclick={confirmDelete}>
				{#if deleting}<Loader2 class="h-4 w-4 animate-spin" />{/if}
				ลบข้อสอบ
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
