export const _meta = {
    menu: {
        title: 'จัดตารางสอน',
        icon: 'CalendarDays',
        group: 'academic',
        permission: 'academic_course_plan.manage.all',
        order: 51,
        user_type: 'staff'
    }
};

export const load = async () => {
    return {
        title: _meta.menu.title
    };
};
