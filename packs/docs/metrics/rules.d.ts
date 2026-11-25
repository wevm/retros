/**
 * Declares the textlint rules that ship without types. Each module exports one textlint rule.
 *
 * @module
 */

declare module 'textlint-rule-en-max-word-count' {
  import type { TextlintKernelRule } from '@textlint/kernel'

  const rule: TextlintKernelRule['rule']
  export default rule
}

declare module 'textlint-rule-write-good' {
  import type { TextlintKernelRule } from '@textlint/kernel'

  const rule: TextlintKernelRule['rule']
  export default rule
}
