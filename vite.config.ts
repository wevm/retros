/**
 * Configures Vite Plus, which formats and lints the TypeScript in this repository. Markdown and
 * TOML keep their authored layout, and the examples keep their own tools, since their scripts
 * measure their own formatting and lint.
 * @module
 */
import { defineConfig } from 'vite-plus'

const ignorePatterns = [
  '**/*.md',
  '**/*.mdx',
  '**/*.toml',
  '**/dist/**',
  'crates/**',
  'examples/**',
  'pnpm-lock.yaml',
  'target/**',
]

export default defineConfig({
  fmt: {
    ignorePatterns,
    printWidth: 80,
    semi: false,
    singleQuote: true,
    sortPackageJson: false,
  },
  lint: {
    categories: { correctness: 'error' },
    ignorePatterns,
    // `pnpm check:types` checks types, so linting stays syntax-only.
    options: { typeAware: false, typeCheck: false },
    rules: { 'no-unused-vars': 'error' },
  },
})
