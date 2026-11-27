/**
 * Measures focus companions and reduced-motion overrides in the authored stylesheet.
 * Selectors in different nesting or media contexts are separate items.
 * @module
 */
import * as PostCSS from 'postcss'
import parseSelector from 'postcss-selector-parser'

/** A selector and the location of its first unmet requirement, if any. */
type Finding = {
  /** Selector qualified by its surrounding conditions. */
  item: string
  /** Missing companion or override, with its source line. */
  problem?: string | undefined
}

const motionProperty =
  /^(?:animation(?:-name|-duration)?|transition(?:-property|-duration)?)$/
const preference =
  /^(?:(?:screen|all)\s+and\s+)?\(\s*prefers-reduced-motion\s*(?::\s*(reduce|no-preference)\s*)?\)$/i

/** Measures each positive hover selector and each selector that specifies motion. */
export function measure(css: string, file: string) {
  const root = PostCSS.parse(css, { from: file })
  const selectors = new Set<string>()
  const hovers = new Map<
    string,
    { companions: readonly string[]; line: number }
  >()

  root.walkRules((rule) => {
    const scope = ancestors(rule).map(context).join(' / ')
    const qualify = (selector: string) =>
      [scope, selector].filter(Boolean).join(' / ')

    for (const branch of parseSelector().astSync(rule.selector, {
      lossless: false,
    }).nodes) {
      const item = qualify(branch.toString())
      selectors.add(item)
      let hovered = false
      const companions = [':focus', ':focus-visible'].map((focus) => {
        const clone = branch.clone()
        clone.walkPseudos((pseudo) => {
          if (pseudo.value.toLowerCase() !== ':hover') return
          for (let at = pseudo.parent; at; at = at.parent) {
            if (at.type === 'pseudo' && at.value.toLowerCase() === ':not')
              return
          }
          hovered = true
          const alternative = pseudo.parent
          const union = alternative?.parent
          if (
            alternative?.type === 'selector' &&
            union?.type === 'pseudo' &&
            [':is', ':where'].includes(union.value.toLowerCase())
          ) {
            const companion = alternative.clone()
            companion.at(alternative.index(pseudo)).value = focus
            if (
              union.nodes.some(
                (option) => option.toString() === companion.toString(),
              )
            )
              return
          }
          pseudo.value = focus
        })
        return qualify(clone.toString())
      })
      if (hovered)
        hovers.set(item, { companions, line: rule.source!.start!.line })
    }
  })

  const focus: Finding[] = []
  for (const [item, hover] of hovers) {
    const paired = hover.companions.some((selector) => selectors.has(selector))
    focus.push({
      item,
      problem: paired
        ? undefined
        : `line ${hover.line}: ${item} has no matching :focus or :focus-visible selector`,
    })
  }

  const motion = new Map<string, Map<string, number>>()
  const overrides = new Map<string, Set<string>>()
  root.walkDecls((decl) => {
    const prop = decl.prop.toLowerCase()
    if (!motionProperty.test(prop)) return
    const family = prop.startsWith('animation') ? 'animation' : 'transition'

    const contexts = ancestors(decl)
    const rule = contexts.findLast((ancestor) => ancestor.type === 'rule')
    if (!rule) return
    const media = contexts.filter(
      (ancestor): ancestor is PostCSS.AtRule =>
        ancestor.type === 'atrule' && ancestor.name.toLowerCase() === 'media',
    )
    if (
      media.some(
        (query) =>
          preference.exec(query.params)?.[1]?.toLowerCase() === 'no-preference',
      )
    )
      return

    const reduce = media.filter((query) => preference.test(query.params))
    const scope = contexts
      .filter(
        (ancestor) =>
          ancestor !== rule && !reduce.some((query) => query === ancestor),
      )
      .map(context)
    const disables = /^(?:none|0(?:\.0+)?(?:ms|s))$/i.test(decl.value.trim())
    for (const selector of rule.selectors) {
      const item = [...scope, selector].join(' / ')
      if (reduce.length) {
        const disabled = overrides.get(item) ?? new Set<string>()
        if (disables) disabled.add(family)
        else disabled.delete(family)
        overrides.set(item, disabled)
      } else if (!disables) {
        const properties = motion.get(item) ?? new Map<string, number>()
        properties.set(family, decl.source!.start!.line)
        motion.set(item, properties)
      }
    }
  })

  const reduced: Finding[] = []
  for (const [item, properties] of motion) {
    for (const [family, line] of properties) {
      const disabled = overrides.get(item)?.has(family)
      reduced.push({
        item: `${item} ${family}`,
        problem: disabled
          ? undefined
          : `line ${line}: ${item} has no matching reduced-motion override for ${family}`,
      })
    }
  }
  return { 'focus-styles': focus, 'reduced-motion': reduced }
}

/** Returns enclosing rules and conditions in source nesting order. */
function ancestors(
  node: PostCSS.ChildNode,
): readonly (PostCSS.AtRule | PostCSS.Rule)[] {
  const parts: (PostCSS.AtRule | PostCSS.Rule)[] = []
  for (
    let at = node.parent;
    at instanceof PostCSS.Rule || at instanceof PostCSS.AtRule;
    at = at.parent
  ) {
    parts.push(at)
  }
  return parts.reverse()
}

function context(node: PostCSS.AtRule | PostCSS.Rule): string {
  return node.type === 'atrule'
    ? `@${node.name} ${node.params}`.trim()
    : node.selector
}
