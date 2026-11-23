/**
 * Defines the site's tokens, global styles, and self-hosted fonts. Colors come from Zyzz's default
 * tokens as light and dark pairs, so the site follows the reader's color scheme.
 * @module
 */
import { defineConfig } from 'zyzz'
import { tokens } from 'zyzz/default'
import { fontFace, global } from 'zyzz/web'

/** Helpers bound to the site tokens. */
export const { style, variants, vars } = defineConfig({
  id: 'site',
  vars: tokens,
})

fontFace({
  fontDisplay: 'swap',
  fontFamily: 'Geist',
  fontStyle: 'normal',
  fontWeight: '100 900',
  src: 'url("@fontsource-variable/geist/files/geist-latin-wght-normal.woff2") format("woff2")',
})

fontFace({
  fontDisplay: 'swap',
  fontFamily: 'Geist Mono',
  fontStyle: 'normal',
  fontWeight: '100 900',
  src: 'url("@fontsource-variable/geist-mono/files/geist-mono-latin-wght-normal.woff2") format("woff2")',
})

global({
  'html, body': { backgroundColor: vars.color.background.surface },
  body: {
    color: vars.color.foreground,
    fontFamily: vars.fontFamily.sans,
    MozOsxFontSmoothing: 'grayscale',
    WebkitFontSmoothing: 'antialiased',
  },
  'code, kbd, pre': { fontFamily: vars.fontFamily.mono },
})
