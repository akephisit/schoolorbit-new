import amsRegular from 'katex/dist/fonts/KaTeX_AMS-Regular.woff2?inline';
import caligraphicBold from 'katex/dist/fonts/KaTeX_Caligraphic-Bold.woff2?inline';
import caligraphicRegular from 'katex/dist/fonts/KaTeX_Caligraphic-Regular.woff2?inline';
import frakturBold from 'katex/dist/fonts/KaTeX_Fraktur-Bold.woff2?inline';
import frakturRegular from 'katex/dist/fonts/KaTeX_Fraktur-Regular.woff2?inline';
import mainBold from 'katex/dist/fonts/KaTeX_Main-Bold.woff2?inline';
import mainBoldItalic from 'katex/dist/fonts/KaTeX_Main-BoldItalic.woff2?inline';
import mainItalic from 'katex/dist/fonts/KaTeX_Main-Italic.woff2?inline';
import mainRegular from 'katex/dist/fonts/KaTeX_Main-Regular.woff2?inline';
import mathBoldItalic from 'katex/dist/fonts/KaTeX_Math-BoldItalic.woff2?inline';
import mathItalic from 'katex/dist/fonts/KaTeX_Math-Italic.woff2?inline';
import sansSerifBold from 'katex/dist/fonts/KaTeX_SansSerif-Bold.woff2?inline';
import sansSerifItalic from 'katex/dist/fonts/KaTeX_SansSerif-Italic.woff2?inline';
import sansSerifRegular from 'katex/dist/fonts/KaTeX_SansSerif-Regular.woff2?inline';
import scriptRegular from 'katex/dist/fonts/KaTeX_Script-Regular.woff2?inline';
import size1Regular from 'katex/dist/fonts/KaTeX_Size1-Regular.woff2?inline';
import size2Regular from 'katex/dist/fonts/KaTeX_Size2-Regular.woff2?inline';
import size3Regular from 'katex/dist/fonts/KaTeX_Size3-Regular.woff2?inline';
import size4Regular from 'katex/dist/fonts/KaTeX_Size4-Regular.woff2?inline';
import typewriterRegular from 'katex/dist/fonts/KaTeX_Typewriter-Regular.woff2?inline';

function fontFace(
	family: string,
	url: string,
	options: { style?: 'normal' | 'italic'; weight?: 400 | 700 } = {}
) {
	return `@font-face{font-display:block;font-family:${JSON.stringify(family)};font-style:${options.style ?? 'normal'};font-weight:${options.weight ?? 400};src:url(${JSON.stringify(url)}) format("woff2")}`;
}

export const katexFontEmbedCss = [
	fontFace('KaTeX_AMS', amsRegular),
	fontFace('KaTeX_Caligraphic', caligraphicBold, { weight: 700 }),
	fontFace('KaTeX_Caligraphic', caligraphicRegular),
	fontFace('KaTeX_Fraktur', frakturBold, { weight: 700 }),
	fontFace('KaTeX_Fraktur', frakturRegular),
	fontFace('KaTeX_Main', mainBold, { weight: 700 }),
	fontFace('KaTeX_Main', mainBoldItalic, { style: 'italic', weight: 700 }),
	fontFace('KaTeX_Main', mainItalic, { style: 'italic' }),
	fontFace('KaTeX_Main', mainRegular),
	fontFace('KaTeX_Math', mathBoldItalic, { style: 'italic', weight: 700 }),
	fontFace('KaTeX_Math', mathItalic, { style: 'italic' }),
	fontFace('KaTeX_SansSerif', sansSerifBold, { weight: 700 }),
	fontFace('KaTeX_SansSerif', sansSerifItalic, { style: 'italic' }),
	fontFace('KaTeX_SansSerif', sansSerifRegular),
	fontFace('KaTeX_Script', scriptRegular),
	fontFace('KaTeX_Size1', size1Regular),
	fontFace('KaTeX_Size2', size2Regular),
	fontFace('KaTeX_Size3', size3Regular),
	fontFace('KaTeX_Size4', size4Regular),
	fontFace('KaTeX_Typewriter', typewriterRegular)
].join('\n');
