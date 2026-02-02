export const _meta = {
    menu: {
        title: 'อาคารสถานที่',
        icon: 'School', // Changed to School icon which is more meaningful than Building (generic)
        group: 'general_admin',
        permission: 'facility.read.all',
        order: 10,
        user_type: 'staff'
    }
};

export const load = async () => {
    return {
        title: 'จัดการอาคารและห้องเรียน'
    };
};
