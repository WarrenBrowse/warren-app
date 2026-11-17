import * as grpcTypes from 'management-interface/management-interface/grpc-types';
import { describe, expect, it } from 'vitest';

import { convertFromApiAccessMethodSettings } from '../../src/main/grpc-type-convertions';

function setting(id: string, name: string, accessMethod: grpcTypes.AccessMethod) {
  return new grpcTypes.AccessMethodSetting()
    .setId(new grpcTypes.UUID().setValue(id))
    .setName(name)
    .setEnabled(true)
    .setAccessMethod(accessMethod);
}

describe('convertFromApiAccessMethodSettings', () => {
  it('reads the direct method and the custom proxies, and nothing else', () => {
    const direct = new grpcTypes.AccessMethod().setDirect(new grpcTypes.AccessMethod.Direct());
    const shadowsocks = new grpcTypes.AccessMethod().setCustom(
      new grpcTypes.CustomProxy().setShadowsocks(
        new grpcTypes.Shadowsocks()
          .setIp('192.0.2.10')
          .setPort(443)
          .setPassword('secret')
          .setCipher('aes-256-gcm'),
      ),
    );
    const proto = new grpcTypes.ApiAccessMethodSettings()
      .setDirect(setting('direct-id', 'Direct', direct))
      .setCustomList([setting('custom-id', 'Office', shadowsocks)]);

    expect(convertFromApiAccessMethodSettings(proto)).toEqual({
      direct: { id: 'direct-id', name: 'Direct', enabled: true, type: 'direct' },
      custom: [
        {
          id: 'custom-id',
          name: 'Office',
          enabled: true,
          type: 'shadowsocks',
          ip: '192.0.2.10',
          port: 443,
          password: 'secret',
          cipher: 'aes-256-gcm',
        },
      ],
    });
  });
});
