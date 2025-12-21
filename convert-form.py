#!/usr/bin/env python3
"""
Convert staff/new form to use shadcn-svelte components
Carefully replace only form elements, preserve everything else
"""
import re

# Read the file
with open('frontend-school/src/routes/(app)/staff/new/+page.svelte', 'r', encoding='utf-8') as f:
    content = f.read()

# 1. Add imports after line 4 (after Button import)
import_addition = '''import { Input } from '$lib/components/ui/input';
\timport { Label } from '$lib/components/ui/label';
\timport { Textarea } from '$lib/components/ui/textarea';
\timport * as Select from '$lib/components/ui/select';
\t'''

# Find the Button import line and add after it
content = content.replace(
    "import { Button } from '$lib/components/ui/button';",
    "import { Button } from '$lib/components/ui/button';\n\t" + import_addition
)

# 2. Replace input tags (but NOT checkbox or radio types)
# Replace text inputs
content = re.sub(
    r'<input\s+type="(text|email|tel|password|date)"\s+',
    r'<Input type="\1" ',
    content
)

# 3. Replace textarea tags
content = re.sub(
    r'<textarea\s+',
    r'<Textarea ',
    content
)
content = re.sub(
    r'></textarea>',
    r'/>',
    content
)

# 4. Remove unnecessary classes from Input/Textarea
content = re.sub(
    r'class="w-full px-3 py-2 border border-border rounded-md"',
    '',
    content
)

# 5. Replace specific select elements with Select component
# This is more complex, so we'll do it per select

# Select 1: Title (คำนำหน้า)
title_select_old = r'''<select
\s+bind:value=\{formData\.title\}
\s+class="[^"]*"
\s*>
\s*<option value="นาย">นาย</option>
\s*<option value="นาง">นาง</option>
\s*<option value="นางสาว">นางสาว</option>
\s*<option value="ดร\.">ดร\.</option>
\s*<option value="ศ\.">ศ\.</option>
\s*<option value="รศ\.">รศ\.</option>
\s*<option value="ผศ\.">ผศ\.</option>
\s*</select>'''

title_select_new = '''<Select.Root type="single" bind:value={formData.title}>
\t\t\t\t\t\t\t<Select.Trigger>{formData.title || 'เลือกคำนำหน้า'}</Select.Trigger>
\t\t\t\t\t\t\t<Select.Content>
\t\t\t\t\t\t\t\t<Select.Item value="นาย">นาย</Select.Item>
\t\t\t\t\t\t\t\t<Select.Item value="นาง">นาง</Select.Item>
\t\t\t\t\t\t\t\t<Select.Item value="นางสาว">นางสาว</Select.Item>
\t\t\t\t\t\t\t\t<Select.Item value="ดร.">ดร.</Select.Item>
\t\t\t\t\t\t\t\t<Select.Item value="ศ.">ศ.</Select.Item>
\t\t\t\t\t\t\t\t<Select.Item value="รศ.">รศ.</Select.Item>
\t\t\t\t\t\t\t\t<Select.Item value="ผศ.">ผศ.</Select.Item>
\t\t\t\t\t\t\t</Select.Content>
\t\t\t\t\t\t</Select.Root>'''

content = re.sub(title_select_old, title_select_new, content, flags=re.MULTILINE)

print("✅ Conversion completed!")
print("Writing to file...")

# Write back
with open('frontend-school/src/routes/(app)/staff/new/+page.svelte', 'w', encoding='utf-8') as f:
    f.write(content)

print("✅ Done!")
