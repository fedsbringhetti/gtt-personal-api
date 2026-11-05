import { describe, expect, it } from 'vitest'
import { readPayload } from './pushPayload'

describe('readPayload', () => {
  it('reads a full payload', () => {
    expect(
      readPayload(() => ({ title: 'Linea 9', body: 'Tra 3 min', url: '/fermata/39', tag: 'rule-1' })),
    ).toEqual({ title: 'Linea 9', body: 'Tra 3 min', url: '/fermata/39', tag: 'rule-1' })
  })

  it('falls back to a generic notification', () => {
    expect(readPayload(() => ({}))).toEqual({ title: 'InTime', body: '', url: '/', tag: undefined })
    expect(readPayload(() => undefined)).toEqual({ title: 'InTime', body: '', url: '/', tag: undefined })
    expect(readPayload(() => null)).toEqual({ title: 'InTime', body: '', url: '/', tag: undefined })
  })

  it('survives bad JSON', () => {
    expect(
      readPayload(() => {
        throw new SyntaxError('bad json')
      }),
    ).toEqual({ title: 'InTime', body: '', url: '/' })
  })

  it('ignores a url that is not a path inside the app', () => {
    expect(readPayload(() => { return { url: '//evil.com' } }).url).toBe('/')
    expect(readPayload(() => ({ url: 'https://evil.com' })).url).toBe('/')
    expect(readPayload(() => ({ url: 42 })).url).toBe('/')
  })

  it('ignores fields of the wrong type', () => {
    expect(readPayload(() => ({ title: 1, body: 2, tag: 3 }))).toEqual({
      title: 'InTime',
      body: '',
      url: '/',
      tag: undefined,
    })
  })
})
