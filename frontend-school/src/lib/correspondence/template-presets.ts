import type { ProcessTemplate, ProcessTemplateStep } from '#lib/api/correspondence.js';
export type TemplateSeed = Pick<ProcessTemplate, 'title' | 'steps'>;
function step(title: string, kind: ProcessTemplateStep['kind']): ProcessTemplateStep {
	return {
		title,
		kind,
		completionPolicy: 'all',
		recipients: [],
		requiresSignature: false,
		requiresComment: false,
		requiresEvidence: false,
		requiresDelegatedAuthority: false
	};
}
export const templatePresets: TemplateSeed[] = [
	{ title: 'แจ้งเพื่อทราบ', steps: [step('รับทราบหนังสือ', 'acknowledge')] },
	{
		title: 'เสนออนุมัติ',
		steps: [step('เสนอความเห็น', 'review'), step('พิจารณาอนุมัติ', 'approve')]
	},
	{ title: 'มอบหมายดำเนินการ', steps: [step('ดำเนินการตามมอบหมาย', 'execute')] }
];
export function copyTemplateSeed(template: TemplateSeed): TemplateSeed {
	return {
		title: template.title,
		steps: template.steps.map((step) => ({
			...step,
			recipients: (step.recipients ?? []).map((recipient) => ({ ...recipient }))
		}))
	};
}
