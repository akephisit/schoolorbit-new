import { apiClient, requireApiData } from '$lib/api/client';

export type QuestionType = 'single_choice' | 'multiple_choice' | 'short_answer' | 'essay';
export type QuestionDifficulty = 'easy' | 'medium' | 'hard';
export type QuestionStatus = 'draft' | 'ready' | 'archived';

export type RichTextMark = { type: 'bold' } | { type: 'italic' };

export type RichInlineNode =
	| { type: 'text'; text: string; marks?: RichTextMark[] }
	| { type: 'inline_math'; attrs: { latex: string } }
	| { type: 'hardBreak' };

export type RichContentBlock =
	| { type: 'paragraph'; content?: RichInlineNode[] }
	| { type: 'math_block'; attrs: { latex: string } }
	| {
			type: 'image';
			attrs: {
				fileId: string;
				altText: string | null;
				caption: string | null;
				alignment: 'left' | 'center' | 'right';
				widthPercent: number;
			};
	  };

export interface RichContent {
	schemaVersion: 1;
	document: {
		type: 'doc';
		content: RichContentBlock[];
	};
}

export interface QuestionChoice {
	id: string;
	questionId: string;
	label: string;
	content: RichContent;
	isCorrect: boolean;
	sortOrder: number;
}

export interface QuestionSummary {
	id: string;
	subjectId?: string | null;
	ownerUserId: string;
	questionType: QuestionType;
	difficulty: QuestionDifficulty;
	points: number;
	stemContent: RichContent;
	explanationContent?: RichContent | null;
	rubricContent?: RichContent | null;
	tags: string[];
	status: QuestionStatus;
	subjectCode?: string | null;
	subjectNameTh?: string | null;
	subjectNameEn?: string | null;
	subjectGroupId?: string | null;
	subjectGroupName?: string | null;
	choiceCount: number;
	correctChoiceCount: number;
	canManage: boolean;
	createdAt: string;
	updatedAt: string;
}

export interface QuestionFile {
	id: string;
	url: string;
	thumbnailUrl?: string | null;
}

export interface QuestionDetail extends QuestionSummary {
	choices: QuestionChoice[];
	files: QuestionFile[];
}

export interface UpsertQuestionChoiceRequest {
	id?: string | null;
	label: string;
	content: RichContent;
	isCorrect: boolean;
	sortOrder: number;
}

export interface UpsertQuestionRequest {
	subjectId: string;
	questionType: QuestionType;
	difficulty: QuestionDifficulty;
	points: number;
	stemContent: RichContent;
	explanationContent?: RichContent | null;
	rubricContent?: RichContent | null;
	tags: string[];
	status: QuestionStatus;
	choices: UpsertQuestionChoiceRequest[];
}

export interface QuestionBankListQuery {
	subjectId?: string;
	questionType?: QuestionType | 'all';
	difficulty?: QuestionDifficulty | 'all';
	status?: QuestionStatus | 'all';
	tag?: string;
	search?: string;
	page?: number;
	pageSize?: number;
}

export interface QuestionBankSummary {
	total: number;
	choice: number;
	written: number;
	ready: number;
}

export interface QuestionBankPage {
	items: QuestionSummary[];
	total: number;
	page: number;
	pageSize: number;
	totalPages: number;
	summary: QuestionBankSummary;
}

export interface QuestionBankSubjectOption {
	id: string;
	code: string;
	nameTh: string;
	nameEn?: string | null;
	subjectGroupId?: string | null;
	subjectGroupName?: string | null;
	canCreate: boolean;
}

export interface QuestionBankOptions {
	subjects: QuestionBankSubjectOption[];
}

function questionBankQueryString(query: QuestionBankListQuery = {}) {
	const params = new URLSearchParams();
	if (query.subjectId) params.set('subjectId', query.subjectId);
	if (query.questionType && query.questionType !== 'all')
		params.set('questionType', query.questionType);
	if (query.difficulty && query.difficulty !== 'all') params.set('difficulty', query.difficulty);
	if (query.status && query.status !== 'all') params.set('status', query.status);
	if (query.tag?.trim()) params.set('tag', query.tag.trim());
	if (query.search?.trim()) params.set('search', query.search.trim());
	if (query.page) params.set('page', String(query.page));
	if (query.pageSize) params.set('pageSize', String(query.pageSize));
	const value = params.toString();
	return value ? `?${value}` : '';
}

export async function listQuestionBankQuestions(
	query: QuestionBankListQuery = {}
): Promise<QuestionBankPage> {
	const response = await apiClient.get<QuestionBankPage>(
		`/api/academic/question-bank/questions${questionBankQueryString(query)}`
	);
	return requireApiData(response, 'โหลดคลังข้อสอบไม่สำเร็จ');
}

export async function getQuestionBankOptions(): Promise<QuestionBankOptions> {
	const response = await apiClient.get<QuestionBankOptions>('/api/academic/question-bank/options');
	return requireApiData(response, 'โหลดตัวเลือกรายวิชาไม่สำเร็จ');
}

export async function getQuestionBankQuestion(id: string): Promise<QuestionDetail> {
	const response = await apiClient.get<QuestionDetail>(
		`/api/academic/question-bank/questions/${id}`
	);
	return requireApiData(response, 'โหลดข้อสอบไม่สำเร็จ');
}

export async function createQuestionBankQuestion(
	payload: UpsertQuestionRequest
): Promise<QuestionDetail> {
	const response = await apiClient.post<QuestionDetail>(
		'/api/academic/question-bank/questions',
		payload
	);
	return requireApiData(response, 'บันทึกข้อสอบไม่สำเร็จ');
}

export async function updateQuestionBankQuestion(
	id: string,
	payload: UpsertQuestionRequest
): Promise<QuestionDetail> {
	const response = await apiClient.put<QuestionDetail>(
		`/api/academic/question-bank/questions/${id}`,
		payload
	);
	return requireApiData(response, 'แก้ไขข้อสอบไม่สำเร็จ');
}

export async function deleteQuestionBankQuestion(id: string): Promise<void> {
	const response = await apiClient.delete<Record<string, never>>(
		`/api/academic/question-bank/questions/${id}`
	);
	if (!response.success) throw new Error(response.error || 'ลบข้อสอบไม่สำเร็จ');
}
