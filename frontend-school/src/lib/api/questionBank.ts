import { apiClient, BACKEND_URL, requireApiData, type ApiRequestOptions } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];
type ExportQuestionBankDataRequest =
	operations['exportQuestionBankData']['requestBody']['content']['application/json'];
type QuestionId = operations['getQuestionBankQuestion']['parameters']['path']['id'];

export type RichTextMark = Schemas['RichTextMark'];
export type RichInlineNode = Schemas['RichInlineNode'];
export type RichContentBlock = Schemas['RichBlockNode'];
export type RichContent = Schemas['RichContent'];
export type QuestionChoice = Schemas['QuestionChoice'];
export type QuestionSummary = Schemas['QuestionSummary'];
export type QuestionFile = Schemas['QuestionFile'];
export type QuestionDetail = Schemas['QuestionDetail'];
export type UpsertQuestionChoiceRequest = Schemas['UpsertQuestionChoiceRequest'];
export type UpsertQuestionRequest =
	operations['createQuestionBankQuestion']['requestBody']['content']['application/json'];
export type QuestionBankListQuery = NonNullable<
	operations['listQuestionBankQuestions']['parameters']['query']
>;
export type QuestionBankSummary = Schemas['QuestionBankSummary'];
export type QuestionBankPage = Schemas['QuestionBankPage'];
export type QuestionBankSubjectOption = Schemas['QuestionBankSubjectOption'];
export type QuestionBankOptions = Schemas['QuestionBankOptions'];
export type QuestionType = QuestionSummary['questionType'];
export type QuestionDifficulty = QuestionSummary['difficulty'];
export type QuestionStatus = QuestionSummary['status'];

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

export async function getQuestionBankQuestion(id: QuestionId): Promise<QuestionDetail> {
	const response = await apiClient.get<QuestionDetail>(
		`/api/academic/question-bank/questions/${encodeURIComponent(id)}`
	);
	return requireApiData(response, 'โหลดข้อสอบไม่สำเร็จ');
}

export async function exportQuestionBankData(
	questionIds: ExportQuestionBankDataRequest['questionIds'],
	options: ApiRequestOptions = {}
): Promise<QuestionDetail[]> {
	const body = { questionIds } satisfies ExportQuestionBankDataRequest;
	const response = await apiClient.post<QuestionDetail[]>(
		'/api/academic/question-bank/questions/export-data',
		body,
		options
	);
	return requireApiData(response, 'โหลดข้อมูลข้อสอบสำหรับส่งออกไม่สำเร็จ');
}

export async function getQuestionBankQuestionFile(
	questionId: string,
	fileId: string
): Promise<Blob> {
	const response = await apiClient.getBlob(
		`/api/academic/question-bank/questions/${encodeURIComponent(questionId)}/files/${encodeURIComponent(fileId)}`
	);
	return requireApiData(response, 'ดาวน์โหลดรูปประกอบข้อสอบไม่สำเร็จ');
}

export function questionBankFileContentUrl(questionId: string, fileId: string): string {
	return `${BACKEND_URL}/api/academic/question-bank/questions/${encodeURIComponent(questionId)}/files/${encodeURIComponent(fileId)}`;
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
