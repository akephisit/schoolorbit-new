interface RequestOptions {
	signal?: AbortSignal;
}

export async function loadQuestionBankExportData<Detail>(
	deps: {
		exportQuestionBankData: (questionIds: string[], options?: RequestOptions) => Promise<Detail[]>;
	},
	questionIds: string[],
	signal: AbortSignal
): Promise<Detail[]> {
	return deps.exportQuestionBankData([...questionIds], { signal });
}
