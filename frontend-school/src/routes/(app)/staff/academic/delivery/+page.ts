import type { PageLoad } from './$types';
import {
	LEARNING_DELIVERY_PAGE_DEPENDENCY,
	readLearningDeliveryRouteContext
} from '$lib/academic/learning-delivery-page';
import { getLearningDeliveryPageView } from '$lib/api/learning-delivery';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required',
	menu: {
		title: 'รายวิชาและกิจกรรมที่เปิดสอน',
		icon: 'Workflow',
		group: 'academic_delivery',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.LEARNING_OFFERING
	}
};

export const load: PageLoad = async ({ depends, fetch, url }) => {
	depends(LEARNING_DELIVERY_PAGE_DEPENDENCY);
	const context = readLearningDeliveryRouteContext(url);
	return {
		title: _meta.menu.title,
		context,
		pageView: context
			? await captureRouteLoad(
					getLearningDeliveryPageView(context.academicYearId, context.academicTermId, {
						timetableVersionId: context.timetableVersionId,
						requestFetch: fetch
					}),
					'โหลดหน้าจัดการการเปิดสอนไม่สำเร็จ'
				)
			: null
	};
};
