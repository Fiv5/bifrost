import { describe, it, expect } from 'vitest';
import { getClientIp } from '../security';

describe('getClientIp', () => {
  const makeReq = (
    headers: Record<string, string | string[] | undefined> = {},
    remoteAddress?: string,
  ) => ({
    headers,
    socket: { remoteAddress },
  });

  describe('trustForwardedFor = false (default)', () => {
    it('should use socket.remoteAddress', () => {
      const req = makeReq({}, '192.168.1.100');
      expect(getClientIp(req, false)).toBe('192.168.1.100');
    });

    it('should ignore x-forwarded-for header', () => {
      const req = makeReq({ 'x-forwarded-for': '10.0.0.1, 10.0.0.2' }, '192.168.1.100');
      expect(getClientIp(req, false)).toBe('192.168.1.100');
    });

    it('should ignore x-real-ip header', () => {
      const req = makeReq({ 'x-real-ip': '10.0.0.1' }, '192.168.1.100');
      expect(getClientIp(req, false)).toBe('192.168.1.100');
    });

    it('should return unknown when no socket address', () => {
      const req = makeReq({ 'x-forwarded-for': '10.0.0.1' });
      expect(getClientIp(req, false)).toBe('unknown');
    });

    it('should default trustForwardedFor to false', () => {
      const req = makeReq({ 'x-forwarded-for': '10.0.0.1' }, '192.168.1.100');
      expect(getClientIp(req)).toBe('192.168.1.100');
    });
  });

  describe('trustForwardedFor = true', () => {
    it('should prefer x-forwarded-for over socket address', () => {
      const req = makeReq({ 'x-forwarded-for': '10.0.0.1' }, '192.168.1.100');
      expect(getClientIp(req, true)).toBe('10.0.0.1');
    });

    it('should take first IP from x-forwarded-for chain', () => {
      const req = makeReq({ 'x-forwarded-for': '10.0.0.1, 10.0.0.2, 10.0.0.3' }, '192.168.1.100');
      expect(getClientIp(req, true)).toBe('10.0.0.1');
    });

    it('should handle x-forwarded-for as array', () => {
      const req = makeReq({ 'x-forwarded-for': ['10.0.0.1, 10.0.0.2'] }, '192.168.1.100');
      expect(getClientIp(req, true)).toBe('10.0.0.1');
    });

    it('should fallback to x-real-ip if x-forwarded-for absent', () => {
      const req = makeReq({ 'x-real-ip': '10.0.0.5' }, '192.168.1.100');
      expect(getClientIp(req, true)).toBe('10.0.0.5');
    });

    it('should handle x-real-ip as array', () => {
      const req = makeReq({ 'x-real-ip': ['10.0.0.5'] }, '192.168.1.100');
      expect(getClientIp(req, true)).toBe('10.0.0.5');
    });

    it('should prefer x-forwarded-for over x-real-ip', () => {
      const req = makeReq(
        { 'x-forwarded-for': '10.0.0.1', 'x-real-ip': '10.0.0.5' },
        '192.168.1.100',
      );
      expect(getClientIp(req, true)).toBe('10.0.0.1');
    });

    it('should fallback to socket address when no proxy headers', () => {
      const req = makeReq({}, '192.168.1.100');
      expect(getClientIp(req, true)).toBe('192.168.1.100');
    });

    it('should skip empty x-forwarded-for and use socket', () => {
      const req = makeReq({ 'x-forwarded-for': '' }, '192.168.1.100');
      expect(getClientIp(req, true)).toBe('192.168.1.100');
    });
  });
});
