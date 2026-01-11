import type { PageLoad } from './$types';
import { getStaffProfile } from '$lib/api/staff';
import { getAchievements } from '$lib/api/achievement';
import { error } from '@sveltejs/kit';

export const load: PageLoad = async ({ params, fetch }) => {
    const staffId = params.id;

    try {
        const [staffRes, achievementsRes] = await Promise.all([
            getStaffProfile(staffId),
            getAchievements({ user_id: staffId })
        ]);

        if (!staffRes.success || !staffRes.data) {
            throw error(404, 'ไม่พบข้อมูลบุคลากร');
        }

        return {
            staff: staffRes.data,
            achievements: achievementsRes.success && achievementsRes.data ? achievementsRes.data : [],
            title: `${staffRes.data.first_name} ${staffRes.data.last_name}`
        };
    } catch (e) {
        console.error('Error loading staff details:', e);
        throw error(500, 'เกิดข้อผิดพลาดในการโหลดข้อมูล');
    }
};
