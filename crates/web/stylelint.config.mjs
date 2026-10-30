export default {
  plugins: ['./lint/design-tokens.mjs'],
  // Debt belongs in the exact-match ratchet, never in disable comments.
  ignoreDisables: true,
  rules: { 'tickets/design-tokens': true },
  overrides: [{ files: ['style/tokens.css'], rules: { 'tickets/design-tokens': null } }],
};
