import test from 'node:test';
import assert from 'node:assert/strict';
import { invalidValue } from './value-policy.mjs';

test('tokens, keywords, responsive geometry and token arithmetic', () => {
  for (const [property, value] of [
    ['padding', '0 var(--space-4)'], ['margin-inline', 'auto'],
    ['inset-block-start', 'calc(100% + var(--space-2))'],
    ['gap', 'calc(var(--space-1) * 2)'], ['border-radius', '50%'],
    ['color', 'currentColor'], ['background', 'transparent'],
    ['border', '1px solid var(--border)'], ['outline', '2px solid var(--focus)'],
    ['box-shadow', 'var(--shadow-popover)'], ['font-size', 'inherit'],
    ['background', 'color-mix(in srgb, var(--accent) 14%, transparent)'],
    ['color', 'var(--dynamic-color, var(--text))'],
    ['padding', 'env(safe-area-inset-top)'], ['--segments', '4'],
    ['width', '240px'], ['border-width', '1px'],
  ]) assert.equal(invalidValue(property, value), false, `${property}:${value}`);
});

test('all covered properties and shorthands reject raw values', () => {
  for (const [property, value] of [
    ['padding', '16px'], ['scroll-margin-top', '2rem'], ['gap', '.5em'],
    ['inset-inline', '-4px'], ['border-end-start-radius', '6px'],
    ['color', '#fff'], ['background', 'rgb(0 0 0)'], ['fill', 'red'],
    ['stroke', 'hsl(0 0% 0%)'], ['border', '1px solid #fff'],
    ['box-shadow', '0 4px 24px #0003'], ['text-shadow', '0 1px var(--text)'],
    ['font-size', '14px'], ['font', '14px/1.5 sans-serif'],
    ['--local-gap', '4px'], ['--local-color', '#fff'],
  ]) assert.equal(invalidValue(property, value), true, `${property}:${value}`);
});

test('a token or function cannot hide raw values', () => {
  for (const value of ['var(--space-1) 3px', 'calc(var(--space-1) + 3px)', 'calc(4px * 2)', 'var(--space-1, 3px)', 'min(2rem, var(--space-4))', 'var(--x) /* safe */ 2px', 'var(--x']) {
    assert.equal(invalidValue('padding', value), true, value);
  }
  assert.equal(invalidValue('color', 'var(--text, red)'), true);
  assert.equal(invalidValue('background', 'linear-gradient(#fff, var(--bg))'), true);
});
