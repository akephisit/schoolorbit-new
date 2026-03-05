import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => {
    return { title: 'จัดการรอบรับสมัคร', periodId: params.id };
};
