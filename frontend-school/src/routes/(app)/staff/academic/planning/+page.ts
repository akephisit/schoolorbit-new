/**
 * Course Planning Page
 */

export const _meta = {
    menu: {
        title: 'จัดแผนการเรียน',
        icon: 'BookOpen',
        group: 'academic',
        order: 30, // ถัดจาก ห้องเรียน (20)
        user_type: 'staff',
        permission: 'academic_course_plan.read.all'
    }
};

export const load = async () => {
    return {
        title: _meta.menu.title
    };
};
