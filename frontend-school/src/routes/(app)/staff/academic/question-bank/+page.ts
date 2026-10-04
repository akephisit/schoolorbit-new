import type { PageLoad } from './$types';
import { getQuestionBankOptions, listQuestionBankQuestions } from '#lib/api/questionBank.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'คลังข้อสอบ',
		icon: 'BookOpenCheck',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_QUESTION_BANK
	}
};

export const load: PageLoad = ({ fetch }) => {
	return {
		title: _meta.menu.title,
		options: captureRouteLoad(
			getQuestionBankOptions({ requestFetch: fetch }),
			'โหลดตัวเลือกรายวิชาไม่สำเร็จ'
		),
		questionPage: captureRouteLoad(
			listQuestionBankQuestions({ page: 1, pageSize: 20 }, { requestFetch: fetch }),
			'โหลดคลังข้อสอบไม่สำเร็จ'
		)
	};
};
