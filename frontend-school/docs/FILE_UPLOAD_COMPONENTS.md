# File Upload Components

Frontend components for file upload integration with SchoolOrbit backend.

## Components

### 1. ImageUpload.svelte
Generic image upload component with drag-and-drop support.

**Features:**
- 📤 Drag and drop support
- 🖼️ Image preview
- ✅ File validation (type, size)
- 🎨 Beautiful UI with shadcn-svelte
- ♿ Accessible

**Usage:**
```svelte
<script>
import ImageUpload from '$lib/components/forms/ImageUpload.svelte';

let imageUrl = null;

function handleUpload(event) {
  const file = event.detail;
  // Handle file upload
}
</script>

<ImageUpload
  value={imageUrl}
  maxSizeMB={5}
  previewSize="md"
  on:upload={handleUpload}
  on:remove={() => imageUrl = null}
/>
```

**Props:**
- `value` - Current image URL
- `maxSizeMB` - Maximum file size (default: 5)
- `accept` - Accepted file types
- `disabled` - Disable upload
- `previewSize` - Preview size: 'sm' | 'md' | 'lg'

### 2. ProfileImageUpload.svelte
Complete profile image upload with API integration.

**Features:**
- ✨ Automatic upload to backend
- 🔔 Toast notifications
- 🔄 Loading states
- 📦 Complete API integration

**Usage:**
```svelte
<script>
import ProfileImageUpload from '$lib/components/forms/ProfileImageUpload.svelte';

let currentImage = 'https://...';

function handleSuccess(event) {
  const { url, fileId } = event.detail;
  console.log('New image:', url);
}
</script>

<ProfileImageUpload
  {currentImage}
  on:success={handleSuccess}
  on:error={(e) => console.error(e.detail)}
/>
```

### 3. Avatar.svelte
Display user avatar with fallbacks.

**Features:**
- 🖼️ Image display
- 🔤 Initials fallback
- 👤 Icon fallback
- 📐 Multiple sizes
- ⭕ Circle or square shape

**Usage:**
```svelte
<script>
import { Avatar } from '$lib/components/ui/avatar';

const user = {
  name: 'John Doe',
  avatar: 'https://...'
};
</script>

<Avatar
  src={user.avatar}
  initials="JD"
  size="md"
  shape="circle"
/>
```

**Props:**
- `src` - Image URL
- `alt` - Alt text
- `initials` - Fallback initials
- `size` - 'xs' | 'sm' | 'md' | 'lg' | 'xl'
- `shape` - 'circle' | 'square'

## API Services

### uploadProfileImage(file: File)
Upload a profile image to the backend.

```typescript
import { uploadProfileImage } from '$lib/api/files';

const file = event.target.files[0];
const response = await uploadProfileImage(file);
console.log(response.file.url);
```

### listUserFiles()
Get list of user's uploaded files.

```typescript
import { listUserFiles } from '$lib/api/files';

const files = await listUserFiles();
console.log(files.files);
```

### deleteFile(fileId: string)
Delete a file.

```typescript
import { deleteFile } from '$lib/api/files';

await deleteFile(fileId);
```

## Integration Example

### Update Profile Page

```svelte
<script lang="ts">
import ProfileImageUpload from '$lib/components/forms/ProfileImageUpload.svelte';
import { Avatar } from '$lib/components/ui/avatar';

let profile = $state({
  name: 'John Doe',
  email: 'john@example.com',
  avatar: null
});

function handleImageUpdate(event: CustomEvent) {
  const { url } = event.detail;
  profile.avatar = url;
  
  // Save to backend
  await updateProfile({ profile_image_url: url });
}
</script>

<div class="space-y-4">
  <div class="flex items-center gap-4">
    <Avatar 
      src={profile.avatar} 
      initials="JD" 
      size="lg"
    />
    
    <ProfileImageUpload
      currentImage={profile.avatar}
      on:success={handleImageUpdate}
    />
  </div>
</div>
```

## Environment Variables

Add to `.env`:
```bash
VITE_API_URL=http://localhost:8081
```

## File Types

Supported file types for profile images:
- JPG/JPEG
- PNG
- WebP
- GIF

Maximum size: 5 MB (configurable)

## Styling

Components use Tailwind CSS and shadcn-svelte for consistent styling.

Custom classes can be added via `className` prop:
```svelte
<ImageUpload className="custom-class" />
```

## Accessibility

All components are keyboard accessible and include proper ARIA labels.

## Browser Support

- Modern browsers (Chrome, Firefox, Safari, Edge)
- Drag and drop requires HTML5 support

---

**Next Steps:**
1. ✅ Components created
2. ⏳ Integrate into profile pages
3. ⏳ Test file upload flow
4. ⏳ Add loading states to forms
