import stylelint from 'stylelint';
import { invalidValue } from './value-policy.mjs';

export const ruleName = 'tickets/design-tokens';
export const declarationKey = declaration => `${declaration.prop}:${declaration.value}`;
const rule = enabled => (root, result) => {
  if (!stylelint.utils.validateOptions(result, ruleName, { actual: enabled })) return;
  root.walkDecls(declaration => {
    if (!invalidValue(declaration.prop, declaration.value)) return;
    stylelint.utils.report({
      ruleName, result, node: declaration, word: declaration.value,
      message: `Use a design token: ${declarationKey(declaration)}`,
    });
  });
};
rule.ruleName = ruleName;
export default stylelint.createPlugin(ruleName, rule);
