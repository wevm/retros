/**
 * Tests for the date formatters.
 *
 * @module
 */

import { expect, test } from 'bun:test'

import { isoDate, relative, weekday } from './date.js'

const now = new Date('2026-11-22T12:00:00Z')

test('isoDate', () => {
  expect(isoDate(now)).toBe('2026-11-22')
})

test('relative in the past', () => {
  expect(relative(new Date('2026-11-19T12:00:00Z'), now)).toBe('3 days ago')
})

test('relative in the future', () => {
  expect(relative(new Date('2026-11-24T12:00:00Z'), now)).toBe('in 2 days')
})

test('weekday', () => {
  expect(weekday(now)).toBe('Sunday')
})
