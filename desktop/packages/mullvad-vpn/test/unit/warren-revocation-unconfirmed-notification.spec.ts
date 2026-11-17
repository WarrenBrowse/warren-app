import * as grpcTypes from 'management-interface/management-interface/grpc-types';
import { describe, expect, it } from 'vitest';

import { convertFromWarrenStatus } from '../../src/main/grpc-type-convertions';
import { WarrenRevocationUnconfirmedNotificationProvider } from '../../src/renderer/lib/notifications';

describe('Warren unconfirmed revocation banner', () => {
  it('stays hidden while no revocation is pending', () => {
    const provider = new WarrenRevocationUnconfirmedNotificationProvider({
      deviceRevocationUnconfirmed: false,
    });
    expect(provider.mayDisplay()).to.be.false;
  });

  it('says the device was kept when the account did not sign the revocation', () => {
    const provider = new WarrenRevocationUnconfirmedNotificationProvider({
      deviceRevocationUnconfirmed: true,
    });
    expect(provider.mayDisplay()).to.be.true;

    const notification = provider.getInAppNotification();
    expect(notification.indicator).to.equal('error');
    expect(notification.subtitle).to.contain('Nothing was erased');
    expect(notification.action).to.be.undefined;
  });

  it('reads the flag the daemon pushes in its status', () => {
    const status = new grpcTypes.WarrenStatus();
    expect(convertFromWarrenStatus(status).deviceRevocationUnconfirmed).to.be.false;

    status.setDeviceRevocationUnconfirmed(true);
    const decoded = grpcTypes.WarrenStatus.deserializeBinary(status.serializeBinary());

    expect(convertFromWarrenStatus(decoded).deviceRevocationUnconfirmed).to.be.true;
  });
});
