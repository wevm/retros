/**
 * Tests for the number formatters. `percent` has no test yet, which the coverage metric shows.
 *
 * @module
 */

import { expect, test } from 'bun:test'

import { bytes, grouped } from './number.js'

test('grouped', () => {
  expect(grouped(12345)).toBe('12,345')
})

test('bytes', () => {
  expect(bytes(45_600_000)).toBe('45.6 MB')
})
