// Shared by the Stylelint rule and inline Rust style checks. No autofix: choosing
// a semantic token can change independently even when today's computed values match.
const spacing = /^(?:(?:scroll-)?(?:margin|padding)(?:-.+)?|(?:row-|column-)?gap|inset(?:-.+)?|top|right|bottom|left|border-spacing|letter-spacing|word-spacing)$/;
const radius = /^border-(?:radius|(?:top|bottom)-(?:left|right)-radius|(?:start|end)-(?:start|end)-radius)$/;
const colors = /^(?:color|.*-color|fill|stroke|background|background-image|border(?:-(?:top|right|bottom|left|block|inline|block-start|block-end|inline-start|inline-end))?|outline|text-decoration|column-rule)$/;
const keywords = new Set('inherit initial unset revert revert-layer currentcolor transparent none auto normal inset solid dashed dotted double groove ridge hidden outset repeat no-repeat cover contain center left right top bottom padding-box border-box content-box'.split(' '));
const functions = new Set(['calc', 'min', 'max', 'clamp', 'color-mix', 'linear-gradient', 'radial-gradient', 'conic-gradient', 'repeating-linear-gradient', 'repeating-radial-gradient']);
const numeric = /^[+-]?(?:\d*\.)?\d+(?:e[+-]?\d+)?([a-z%]*)$/i;

export function category(property) {
  const p = property.toLowerCase();
  if (spacing.test(p)) return 'spacing';
  if (radius.test(p)) return 'radius';
  if (p === 'font-size' || p === 'font') return 'font';
  if (p === 'box-shadow' || p === 'text-shadow') return 'shadow';
  if (colors.test(p)) return 'color';
  if (p.startsWith('--')) return 'custom';
  return null;
}

// Parse balanced function arguments rather than treating any var()/calc() as
// permission for the entire declaration. This also checks var() fallbacks.
function parts(value) {
  const out = [];
  const re = /\s+|\/\*[\s\S]*?\*\/|([a-z-][\w-]*)\(|([(),/*+])|([^\s(),/*+]+)/igy;
  let offset = 0;
  while (offset < value.length) {
    re.lastIndex = offset;
    const m = re.exec(value);
    if (!m) return null;
    offset = re.lastIndex;
    if (m[1]) {
      let depth = 1;
      const start = offset;
      while (offset < value.length && depth) {
        if (value[offset] === '(') depth++;
        if (value[offset] === ')') depth--;
        offset++;
      }
      if (depth) return null;
      out.push({ name: m[1].toLowerCase(), args: value.slice(start, offset - 1) });
    } else if (m[2] || m[3]) out.push(m[2] || m[3]);
  }
  return out;
}

export function invalidValue(property, value) {
  const kind = category(property);
  if (!kind) return false;
  const border = /^(?:border(?:-(?:top|right|bottom|left|block|inline|block-start|block-end|inline-start|inline-end))?|outline|column-rule)$/.test(property);
  function valid(input, math = false, mixing = false) {
    const nodes = parts(input);
    if (!nodes?.length) return false;
    return nodes.every(node => {
      if (typeof node !== 'string') {
        if (node.name === 'var') {
          const comma = node.args.indexOf(',');
          const name = (comma < 0 ? node.args : node.args.slice(0, comma)).trim();
          return /^--[\w-]+$/.test(name) && (comma < 0 || valid(node.args.slice(comma + 1), math, mixing));
        }
        if (node.name === 'env') return /^safe-area-inset-(top|right|bottom|left)$/.test(node.args.trim());
        if (!functions.has(node.name)) return false;
        return valid(node.args, ['calc', 'min', 'max', 'clamp'].includes(node.name), node.name === 'color-mix');
      }
      const word = node.toLowerCase();
      if (keywords.has(word)) return true;
      if ([',', '/'].includes(word)) return true;
      if (math && ['+', '-', '*'].includes(word)) return true;
      if (mixing && ['in', 'srgb', 'srgb-linear', 'oklab', 'oklch', 'hsl', 'shorter', 'longer', 'hue'].includes(word)) return true;
      const number = word.match(numeric);
      if (!number) return false;
      if (parseFloat(word) === 0) return true;
      const unit = number[1];
      // Percentages express relative geometry/tints. Unitless math scales tokens.
      if (unit === '%') return true;
      if (!unit && (math || kind === 'custom' || (kind === 'font' && property === 'font'))) return true;
      // Border/outline thickness is geometry, but its colour still needs a token.
      if (border && ['px', 'em', 'rem'].includes(unit)) return true;
      return kind === 'custom' && ['ms', 's', 'deg'].includes(unit);
    });
  }
  return !valid(value);
}
