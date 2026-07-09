<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		createQuestionBankQuestion,
		deleteQuestionBankQuestion,
		getQuestionBankQuestion,
		listQuestionBankQuestions,
		updateQuestionBankQuestion,
		type QuestionDetail,
		type QuestionDifficulty,
		type QuestionStatus,
		type QuestionSummary,
		type QuestionType,
		type RichContent,
		type RichContentBlock,
		type UpsertQuestionRequest
	} from '$lib/api/questionBank';
	import {
		getAcademicStructure,
		listSubjects,
		type AcademicStructureData,
		type GradeLevel,
		type Subject
	} from '$lib/api/academic';
	import { uploadFile } from '$lib/api/files';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Textarea } from '$lib/components/ui/textarea';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		BookOpenCheck,
		Edit3,
		Image as ImageIcon,
		Loader2,
		Plus,
		RefreshCw,
		Save,
		Search,
		Sigma,
		Trash2,
		Upload
	} from 'lucide-svelte';

	let { data } = $props();

	type QuestionTypeFilter = QuestionType | 'all';
	type DifficultyFilter = QuestionDifficulty | 'all';
	type StatusFilter = QuestionStatus | 'all';
	type ChoiceDraft = {
		id?: string | null;
		label: string;
		text: string;
		latex: string;
		imageFileId: string;
		imagePreviewUrl: string;
		isCorrect: boolean;
		sortOrder: number;
		uploading: boolean;
	};
	type QuestionDraft = {
		id?: string;
		subjectId: string;
		gradeLevelId: string;
		questionType: QuestionType;
		difficulty: QuestionDifficulty;
		points: number;
		status: QuestionStatus;
		stemText: string;
		stemLatex: string;
		stemImageFileId: string;
		stemImagePreviewUrl: string;
		explanationText: string;
		rubricText: string;
		tagsText: string;
		choices: ChoiceDraft[];
		stemUploading: boolean;
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
	const canCreateQuestion = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_QUESTION_BANK_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL
		)
	);

	let loading = $state(true);
	let loadingQuestions = $state(false);
	let saving = $state(false);
	let deletingId = $state<string | null>(null);
	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let subjects = $state<Subject[]>([]);
	let questions = $state<QuestionSummary[]>([]);
	let selectedSubjectId = $state('');
	let selectedGradeLevelId = $state('');
	let selectedQuestionType = $state<QuestionTypeFilter>('all');
	let selectedDifficulty = $state<DifficultyFilter>('all');
	let selectedStatus = $state<StatusFilter>('all');
	let search = $state('');
	let tag = $state('');
	let editorOpen = $state(false);
	let draft = $state<QuestionDraft>(newDraft());

	const visibleGradeLevels = $derived(
		[...structure.levels].sort((left, right) => {
			const typeOrder = { kindergarten: 1, primary: 2, secondary: 3 };
			return (
				(typeOrder[left.level_type] ?? 99) - (typeOrder[right.level_type] ?? 99) ||
				left.year - right.year
			);
		})
	);
	const summary = $derived({
		total: questions.length,
		choice: questions.filter((question) =>
			question.questionType === 'single_choice' || question.questionType === 'multiple_choice'
		).length,
		written: questions.filter(
			(question) => question.questionType === 'short_answer' || question.questionType === 'essay'
		).length,
		ready: questions.filter((question) => question.status === 'ready').length
	});
	const isChoiceQuestion = $derived(
		draft.questionType === 'single_choice' || draft.questionType === 'multiple_choice'
	);

	onMount(() => {
		void loadInitialData();
	});

	function newChoice(label: string, index: number): ChoiceDraft {
		return {
			label,
			text: '',
			latex: '',
			imageFileId: '',
			imagePreviewUrl: '',
			isCorrect: index === 0,
			sortOrder: index + 1,
			uploading: false
		};
	}

	function defaultChoices() {
		return ['A', 'B', 'C', 'D'].map((label, index) => newChoice(label, index));
	}

	function newDraft(): QuestionDraft {
		return {
			subjectId: '',
			gradeLevelId: '',
			questionType: 'single_choice',
			difficulty: 'medium',
			points: 1,
			status: 'draft',
			stemText: '',
			stemLatex: '',
			stemImageFileId: '',
			stemImagePreviewUrl: '',
			explanationText: '',
			rubricText: '',
			tagsText: '',
			choices: defaultChoices(),
			stemUploading: false
		};
	}

	function contentFromParts(text: string, latex: string, imageFileId: string): RichContent {
		const blocks: RichContentBlock[] = [];
		if (text.trim()) blocks.push({ type: 'paragraph', text: text.trim() });
		if (latex.trim()) blocks.push({ type: 'math', latex: latex.trim(), display: true });
		if (imageFileId) blocks.push({ type: 'image', fileId: imageFileId });
		return { blocks };
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

	function imageFileIdFromContent(content: RichContent | null | undefined) {
		const block = firstBlock(content, 'image');
		return block?.type === 'image' ? block.fileId : '';
	}

	function contentHasImage(content: RichContent | null | undefined) {
		return Boolean(imageFileIdFromContent(content));
	}

	function tagsFromText(value: string) {
		const tags: string[] = [];
		for (const rawTag of value.split(',')) {
			const tag = rawTag.trim().toLowerCase();
			if (tag && !tags.includes(tag)) {
				tags.push(tag);
			}
		}
		return tags;
	}

	function choiceDraftFromDetail(choice: QuestionDetail['choices'][number], index: number): ChoiceDraft {
		return {
			id: choice.id,
			label: choice.label,
			text: textFromContent(choice.content),
			latex: latexFromContent(choice.content),
			imageFileId: imageFileIdFromContent(choice.content),
			imagePreviewUrl: '',
			isCorrect: choice.isCorrect,
			sortOrder: choice.sortOrder || index + 1,
			uploading: false
		};
	}

	function draftFromDetail(question: QuestionDetail): QuestionDraft {
		return {
			id: question.id,
			subjectId: question.subjectId ?? '',
			gradeLevelId: question.gradeLevelId ?? '',
			questionType: question.questionType,
			difficulty: question.difficulty,
			points: question.points,
			status: question.status,
			stemText: textFromContent(question.stemContent),
			stemLatex: latexFromContent(question.stemContent),
			stemImageFileId: imageFileIdFromContent(question.stemContent),
			stemImagePreviewUrl: '',
			explanationText: textFromContent(question.explanationContent),
			rubricText: textFromContent(question.rubricContent),
			tagsText: question.tags.join(', '),
			choices: question.choices.map(choiceDraftFromDetail),
			stemUploading: false
		};
	}

	function detailToSummary(question: QuestionDetail): QuestionSummary {
		const { choices, ...summaryQuestion } = question;
		return {
			...summaryQuestion,
			choiceCount: choices.length,
			correctChoiceCount: choices.filter((choice) => choice.isCorrect).length
		};
	}

	function questionTitle(question: QuestionSummary) {
		const text = textFromContent(question.stemContent);
		const latex = latexFromContent(question.stemContent);
		return text || latex || (contentHasImage(question.stemContent) ? 'โจทย์รูปภาพ' : 'โจทย์');
	}

	function subjectLabel(question: QuestionSummary) {
		return [question.subjectCode, question.subjectNameTh || question.subjectNameEn]
			.filter(Boolean)
			.join(' ') || '-';
	}

	function gradeLevelLabelFromLevel(level: GradeLevel) {
		return level.short_name || level.name;
	}

	function gradeLevelLabel(question: QuestionSummary) {
		if (!question.gradeLevelType || !question.gradeLevelYear) return '-';
		const prefixByType: Record<string, string> = {
			kindergarten: 'อ.',
			primary: 'ป.',
			secondary: 'ม.'
		};
		return `${prefixByType[question.gradeLevelType] ?? ''}${question.gradeLevelYear}`;
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

	function buildPayload(): UpsertQuestionRequest {
		const stemContent = contentFromParts(draft.stemText, draft.stemLatex, draft.stemImageFileId);
		const explanationContent = contentFromParts(draft.explanationText, '', '');
		const rubricContent = contentFromParts(draft.rubricText, '', '');
		return {
			subjectId: draft.subjectId || null,
			gradeLevelId: draft.gradeLevelId || null,
			questionType: draft.questionType,
			difficulty: draft.difficulty,
			points: Number(draft.points) || 0,
			stemContent,
			explanationContent: explanationContent.blocks.length ? explanationContent : null,
			rubricContent: rubricContent.blocks.length ? rubricContent : null,
			tags: tagsFromText(draft.tagsText),
			status: draft.status,
			choices: isChoiceQuestion
				? draft.choices.map((choice, index) => ({
						id: choice.id ?? null,
						label: choice.label,
						content: contentFromParts(choice.text, choice.latex, choice.imageFileId),
						isCorrect: choice.isCorrect,
						sortOrder: index + 1
					}))
				: []
		};
	}

	async function loadInitialData() {
		if (!canReadQuestionBank) {
			loading = false;
			return;
		}
		loading = true;
		try {
			const [structureResponse, subjectsResponse, questionResponse] = await Promise.all([
				getAcademicStructure(),
				listSubjects({ active_only: true, latest_only: true }),
				listQuestionBankQuestions()
			]);
			structure = structureResponse.data;
			subjects = subjectsResponse.data;
			questions = questionResponse;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'โหลดคลังข้อสอบไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function loadQuestions() {
		loadingQuestions = true;
		try {
			questions = await listQuestionBankQuestions({
				subjectId: selectedSubjectId,
				gradeLevelId: selectedGradeLevelId,
				questionType: selectedQuestionType,
				difficulty: selectedDifficulty,
				status: selectedStatus,
				search,
				tag
			});
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'โหลดคลังข้อสอบไม่สำเร็จ');
		} finally {
			loadingQuestions = false;
		}
	}

	function startCreate() {
		draft = newDraft();
		editorOpen = true;
	}

	async function startEdit(question: QuestionSummary) {
		try {
			const detail = await getQuestionBankQuestion(question.id);
			draft = draftFromDetail(detail);
			if (!isChoiceQuestion && draft.choices.length === 0) {
				draft.choices = defaultChoices();
			}
			editorOpen = true;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'โหลดข้อสอบไม่สำเร็จ');
		}
	}

	function handleQuestionTypeChange(value: QuestionType) {
		draft.questionType = value;
		if (value === 'single_choice' || value === 'multiple_choice') {
			if (draft.choices.length < 2) draft.choices = defaultChoices();
			if (value === 'single_choice' && draft.choices.filter((choice) => choice.isCorrect).length !== 1) {
				draft.choices = draft.choices.map((choice, index) => ({ ...choice, isCorrect: index === 0 }));
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
		const nextLabel = String.fromCharCode(65 + draft.choices.length);
		draft.choices = [...draft.choices, newChoice(nextLabel, draft.choices.length)];
	}

	function removeChoice(index: number) {
		if (draft.choices.length <= 2) return;
		draft.choices = draft.choices
			.filter((_, choiceIndex) => choiceIndex !== index)
			.map((choice, choiceIndex) => ({
				...choice,
				sortOrder: choiceIndex + 1,
				isCorrect:
					draft.questionType === 'single_choice'
						? choiceIndex === 0 && !draft.choices.some((item, itemIndex) => itemIndex !== index && item.isCorrect)
							? true
							: choice.isCorrect
						: choice.isCorrect
			}));
	}

	async function uploadDraftImage(event: Event, target: 'stem' | number) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		try {
			if (target === 'stem') {
				draft.stemUploading = true;
			} else {
				draft.choices[target].uploading = true;
			}
			const response = await uploadFile(file, 'course_material', false);
			if (target === 'stem') {
				draft.stemImageFileId = response.file.id;
				draft.stemImagePreviewUrl = response.file.thumbnail_url ?? response.file.url;
			} else {
				draft.choices[target].imageFileId = response.file.id;
				draft.choices[target].imagePreviewUrl = response.file.thumbnail_url ?? response.file.url;
			}
			toast.success('อัปโหลดรูปแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'อัปโหลดรูปไม่สำเร็จ');
		} finally {
			if (target === 'stem') {
				draft.stemUploading = false;
			} else {
				draft.choices[target].uploading = false;
			}
			input.value = '';
		}
	}

	async function saveQuestion() {
		saving = true;
		try {
			const payload = buildPayload();
			const detail = draft.id
				? await updateQuestionBankQuestion(draft.id, payload)
				: await createQuestionBankQuestion(payload);
			const summaryQuestion = detailToSummary(detail);
			if (draft.id) {
				questions = questions.map((question) =>
					question.id === summaryQuestion.id ? summaryQuestion : question
				);
			} else {
				questions = [summaryQuestion, ...questions];
			}
			draft = draftFromDetail(detail);
			editorOpen = false;
			toast.success('บันทึกข้อสอบแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกข้อสอบไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function deleteQuestion(question: QuestionSummary) {
		if (!question.canManage) return;
		deletingId = question.id;
		try {
			await deleteQuestionBankQuestion(question.id);
			questions = questions.filter((item) => item.id !== question.id);
			if (draft.id === question.id) {
				editorOpen = false;
				draft = newDraft();
			}
			toast.success('ลบข้อสอบแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ลบข้อสอบไม่สำเร็จ');
		} finally {
			deletingId = null;
		}
	}
</script>

<PageShell title={data.title} description="จัดการโจทย์ ตัวเลือก เฉลย และเนื้อหาประกอบ">
	{#snippet actions()}
		<div class="flex items-center gap-2">
			<Button variant="outline" onclick={loadQuestions} disabled={loadingQuestions}>
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
			description="ติดต่อผู้ดูแลระบบเพื่อขอสิทธิ์ academic_question_bank"
		/>
	{:else}
		<section class="grid gap-3 md:grid-cols-4">
			<div class="rounded-lg border bg-card p-4">
				<p class="text-sm text-muted-foreground">ทั้งหมด</p>
				<p class="mt-1 text-2xl font-semibold">{summary.total}</p>
			</div>
			<div class="rounded-lg border bg-card p-4">
				<p class="text-sm text-muted-foreground">ตัวเลือก</p>
				<p class="mt-1 text-2xl font-semibold">{summary.choice}</p>
			</div>
			<div class="rounded-lg border bg-card p-4">
				<p class="text-sm text-muted-foreground">เขียนตอบ</p>
				<p class="mt-1 text-2xl font-semibold">{summary.written}</p>
			</div>
			<div class="rounded-lg border bg-card p-4">
				<p class="text-sm text-muted-foreground">พร้อมใช้</p>
				<p class="mt-1 text-2xl font-semibold">{summary.ready}</p>
			</div>
		</section>

		<section class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(420px,0.72fr)]">
			<div class="space-y-4">
				<div class="rounded-lg border bg-card p-4">
					<div class="grid gap-3 md:grid-cols-3 xl:grid-cols-6">
						<div class="md:col-span-2 xl:col-span-2">
							<Label for="question-search">ค้นหา</Label>
							<div class="relative mt-1">
								<Search class="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
								<Input id="question-search" class="pl-9" bind:value={search} />
							</div>
						</div>
						<div>
							<Label>รายวิชา</Label>
							<Select.Root type="single" bind:value={selectedSubjectId}>
								<Select.Trigger class="mt-1 w-full">
									{subjects.find((subject) => subject.id === selectedSubjectId)?.name_th ?? 'ทุกวิชา'}
								</Select.Trigger>
								<Select.Content>
									<Select.Item value="">ทุกวิชา</Select.Item>
									{#each subjects as subject (subject.id)}
										<Select.Item value={subject.id}>{subject.code} {subject.name_th}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
						<div>
							<Label>ระดับชั้น</Label>
							<Select.Root type="single" bind:value={selectedGradeLevelId}>
								<Select.Trigger class="mt-1 w-full">
									{visibleGradeLevels.find((level) => level.id === selectedGradeLevelId)?.short_name ??
										'ทุกระดับ'}
								</Select.Trigger>
								<Select.Content>
									<Select.Item value="">ทุกระดับ</Select.Item>
									{#each visibleGradeLevels as level (level.id)}
										<Select.Item value={level.id}>{gradeLevelLabelFromLevel(level)}</Select.Item>
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
						<div class="md:col-span-2 xl:col-span-2">
							<Label for="question-tag">Tag</Label>
							<Input id="question-tag" class="mt-1" bind:value={tag} />
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
						<div class="flex items-end">
							<LoadingButton class="w-full" loading={loadingQuestions} onclick={loadQuestions}>
								<Search class="h-4 w-4" />
								ค้นหา
							</LoadingButton>
						</div>
					</div>
				</div>

				{#if questions.length === 0}
					<PageState title="ยังไม่มีข้อสอบ" actionLabel="เพิ่มข้อสอบ" onaction={startCreate} />
				{:else}
					<div class="overflow-hidden rounded-lg border bg-card">
						{#each questions as question (question.id)}
							<div class="border-b p-4 last:border-b-0">
								<div class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
									<div class="min-w-0 space-y-2">
										<div class="flex flex-wrap items-center gap-2">
											<Badge variant={statusVariant(question.status)}>{statusLabel(question.status)}</Badge>
											<Badge variant="outline">{typeLabel(question.questionType)}</Badge>
											<Badge variant="secondary">{difficultyLabel(question.difficulty)}</Badge>
											{#if contentHasImage(question.stemContent)}
												<Badge variant="outline">
													<ImageIcon class="h-3 w-3" />
													รูป
												</Badge>
											{/if}
											{#if latexFromContent(question.stemContent)}
												<Badge variant="outline">
													<Sigma class="h-3 w-3" />
													สูตร
												</Badge>
											{/if}
										</div>
										<h2 class="line-clamp-2 text-base font-medium">{questionTitle(question)}</h2>
										<div class="flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
											<span>{subjectLabel(question)}</span>
											<span>{gradeLevelLabel(question)}</span>
											<span>{question.points} คะแนน</span>
											<span>{question.choiceCount} ตัวเลือก</span>
										</div>
										{#if question.tags.length}
											<div class="flex flex-wrap gap-1">
												{#each question.tags as item (item)}
													<Badge variant="outline">{item}</Badge>
												{/each}
											</div>
										{/if}
									</div>
									<div class="flex shrink-0 gap-2">
										<Button variant="outline" size="sm" onclick={() => startEdit(question)}>
											<Edit3 class="h-4 w-4" />
											แก้ไข
										</Button>
										{#if question.canManage}
											<Button
												variant="destructive"
												size="sm"
												disabled={deletingId === question.id}
												onclick={() => deleteQuestion(question)}
											>
												{#if deletingId === question.id}
													<Loader2 class="h-4 w-4 animate-spin" />
												{:else}
													<Trash2 class="h-4 w-4" />
												{/if}
												ลบ
											</Button>
										{/if}
									</div>
								</div>
							</div>
						{/each}
					</div>
				{/if}
			</div>

			<div class="rounded-lg border bg-card p-4">
				{#if !editorOpen}
					<div class="flex min-h-[520px] flex-col items-center justify-center text-center">
						<BookOpenCheck class="mb-3 h-12 w-12 text-muted-foreground" />
						<h2 class="text-lg font-medium">เลือกข้อสอบหรือเพิ่มข้อสอบใหม่</h2>
						{#if canCreateQuestion}
							<Button class="mt-4" onclick={startCreate}>
								<Plus class="h-4 w-4" />
								เพิ่มข้อสอบ
							</Button>
						{/if}
					</div>
				{:else}
					<div class="space-y-5">
						<div class="flex items-center justify-between gap-3">
							<div>
								<h2 class="text-lg font-semibold">{draft.id ? 'แก้ไขข้อสอบ' : 'เพิ่มข้อสอบ'}</h2>
								<p class="text-sm text-muted-foreground">{draft.id ? draft.id : 'ข้อสอบใหม่'}</p>
							</div>
							<Button variant="outline" onclick={() => (editorOpen = false)}>ปิด</Button>
						</div>

						<div class="grid gap-3 md:grid-cols-2">
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
								<Label>รายวิชา</Label>
								<Select.Root type="single" bind:value={draft.subjectId}>
									<Select.Trigger class="mt-1 w-full">
										{subjects.find((subject) => subject.id === draft.subjectId)?.name_th ?? 'ไม่ระบุ'}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="">ไม่ระบุ</Select.Item>
										{#each subjects as subject (subject.id)}
											<Select.Item value={subject.id}>{subject.code} {subject.name_th}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
							<div>
								<Label>ระดับชั้น</Label>
								<Select.Root type="single" bind:value={draft.gradeLevelId}>
									<Select.Trigger class="mt-1 w-full">
										{visibleGradeLevels.find((level) => level.id === draft.gradeLevelId)?.short_name ??
											'ไม่ระบุ'}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="">ไม่ระบุ</Select.Item>
										{#each visibleGradeLevels as level (level.id)}
											<Select.Item value={level.id}>{gradeLevelLabelFromLevel(level)}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
							<div>
								<Label>ความยาก</Label>
								<Select.Root type="single" bind:value={draft.difficulty}>
									<Select.Trigger class="mt-1 w-full">{difficultyLabel(draft.difficulty)}</Select.Trigger>
									<Select.Content>
										{#each difficultyOptions as option (option.value)}
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

						<div class="space-y-3">
							<Label for="stem-text">โจทย์</Label>
							<Textarea id="stem-text" class="min-h-28" bind:value={draft.stemText} />
							<div class="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
								<div>
									<Label for="stem-latex">สมการ</Label>
									<Input id="stem-latex" class="mt-1 font-mono" bind:value={draft.stemLatex} />
								</div>
								<div>
									<Label for="stem-image">รูป</Label>
									<div class="mt-1 flex items-center gap-2">
										<Input
											id="stem-image"
											type="file"
											accept="image/*"
											onchange={(event) => uploadDraftImage(event, 'stem')}
										/>
										{#if draft.stemUploading}
											<Loader2 class="h-4 w-4 animate-spin" />
										{:else if draft.stemImageFileId}
											<ImageIcon class="h-4 w-4 text-muted-foreground" />
										{:else}
											<Upload class="h-4 w-4 text-muted-foreground" />
										{/if}
									</div>
								</div>
							</div>
							{#if draft.stemImagePreviewUrl}
								<img
									src={draft.stemImagePreviewUrl}
									alt=""
									class="max-h-48 rounded-md border object-contain"
								/>
							{/if}
						</div>

						{#if isChoiceQuestion}
							<div class="space-y-3">
								<div class="flex items-center justify-between">
									<Label>ตัวเลือก</Label>
									<Button variant="outline" size="sm" onclick={addChoice}>
										<Plus class="h-4 w-4" />
										เพิ่มตัวเลือก
									</Button>
								</div>
								<div class="space-y-3">
									{#each draft.choices as choice, index (choice.id ?? `${choice.label}-${index}`)}
										<div class="rounded-md border p-3">
											<div class="mb-3 flex items-center gap-2">
												<Input class="h-9 w-16" bind:value={choice.label} />
												<Button
													variant={choice.isCorrect ? 'default' : 'outline'}
													size="sm"
													onclick={() => toggleCorrectChoice(index)}
												>
													เฉลย
												</Button>
												<Button
													class="ml-auto"
													variant="ghost"
													size="sm"
													disabled={draft.choices.length <= 2}
													onclick={() => removeChoice(index)}
												>
													<Trash2 class="h-4 w-4" />
												</Button>
											</div>
											<div class="space-y-2">
												<Textarea class="min-h-20" bind:value={choice.text} />
												<div class="grid gap-2 md:grid-cols-[minmax(0,1fr)_auto]">
													<Input class="font-mono" bind:value={choice.latex} />
													<div class="flex items-center gap-2">
														<Input
															type="file"
															accept="image/*"
															onchange={(event) => uploadDraftImage(event, index)}
														/>
														{#if choice.uploading}
															<Loader2 class="h-4 w-4 animate-spin" />
														{:else if choice.imageFileId}
															<ImageIcon class="h-4 w-4 text-muted-foreground" />
														{/if}
													</div>
												</div>
												{#if choice.imagePreviewUrl}
													<img
														src={choice.imagePreviewUrl}
														alt=""
														class="max-h-36 rounded-md border object-contain"
													/>
												{/if}
											</div>
										</div>
									{/each}
								</div>
							</div>
						{/if}

						<div class="grid gap-3 md:grid-cols-2">
							<div>
								<Label for="explanation-text">เฉลย/คำอธิบาย</Label>
								<Textarea id="explanation-text" class="mt-1 min-h-24" bind:value={draft.explanationText} />
							</div>
							<div>
								<Label for="rubric-text">Rubric</Label>
								<Textarea id="rubric-text" class="mt-1 min-h-24" bind:value={draft.rubricText} />
							</div>
						</div>
						<div>
							<Label for="tags-text">Tags</Label>
							<Input id="tags-text" class="mt-1" bind:value={draft.tagsText} />
						</div>

						<LoadingButton class="w-full" loading={saving} onclick={saveQuestion}>
							<Save class="h-4 w-4" />
							บันทึกข้อสอบ
						</LoadingButton>
					</div>
				{/if}
			</div>
		</section>
	{/if}
</PageShell>
