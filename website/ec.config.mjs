// @ts-check
// Code blocks, through Expressive Code: the two themes in src/styles, on the well, with a faint
// edge and no window chrome. Kept here rather than in astro.config.mjs so the themes, which are
// objects, reach Starlight's <Code> component too.
import { readFileSync } from 'node:fs';
import { defineEcConfig, ExpressiveCodeTheme } from '@astrojs/starlight/expressive-code';

const theme = (/** @type {string} */ name) =>
	ExpressiveCodeTheme.fromJSONString(
		readFileSync(new URL(`./src/styles/${name}.json`, import.meta.url), 'utf8')
	);

export default defineEcConfig({
	themes: [theme('anistream-dark'), theme('anistream-light')],
	// The themes' colours were chosen to pass on the well; leave them as they are.
	minSyntaxHighlightingColorContrast: 0,
	// No title bar unless a block names a file.
	defaultProps: { frame: 'code' },
	styleOverrides: {
		borderRadius: '12px',
		borderWidth: '1px',
		borderColor: 'var(--ani-rule-faint)',
		codeBackground: 'var(--ani-well)',
		codeFontFamily: 'var(--ani-font-mono)',
		codeFontSize: '14px',
		codeLineHeight: '1.7143',
		codePaddingBlock: '14px',
		codePaddingInline: '18px',
		uiFontFamily: 'var(--ani-font-ui)',
		uiFontSize: '13px',
		focusBorder: 'var(--ani-obi)',
		scrollbarThumbColor: 'var(--ani-rule)',
		scrollbarThumbHoverColor: 'var(--ani-dim)',
		frames: {
			shadowColor: 'transparent',
			frameBoxShadowCssValue: 'none',
			editorBackground: 'var(--ani-well)',
			terminalBackground: 'var(--ani-well)',
			editorActiveTabBackground: 'var(--ani-well)',
			editorActiveTabForeground: 'var(--ani-dim)',
			editorActiveTabBorderColor: 'transparent',
			editorActiveTabIndicatorTopColor: 'transparent',
			editorActiveTabIndicatorBottomColor: 'transparent',
			editorTabBarBackground: 'var(--ani-well)',
			editorTabBarBorderColor: 'transparent',
			editorTabBarBorderBottomColor: 'var(--ani-rule-faint)',
			terminalTitlebarBackground: 'var(--ani-well)',
			terminalTitlebarForeground: 'var(--ani-dim)',
			terminalTitlebarBorderBottomColor: 'var(--ani-rule-faint)',
			terminalTitlebarDotsOpacity: '0',
			inlineButtonBackground: 'var(--ani-well)',
			inlineButtonBackgroundIdleOpacity: '1',
			inlineButtonBackgroundHoverOrFocusOpacity: '1',
			inlineButtonBackgroundActiveOpacity: '1',
			inlineButtonForeground: 'var(--ani-dim)',
			inlineButtonBorder: 'transparent',
			inlineButtonBorderOpacity: '0',
			tooltipSuccessBackground: 'var(--ani-raised-selected)',
			tooltipSuccessForeground: 'var(--ani-text)',
		},
	},
});
