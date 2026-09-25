#pragma once

#include <winfw/rules/ifirewallrule.h>

namespace rules::baseline
{

//
// "VPN only for these apps": permits IPv4 traffic outside the tunnel for
// every app. The split tunnel driver, engaged with the physical address as
// its "tunnel" address, hard-blocks the included apps on that address at a
// higher weight in the same sublayer, so they are the only ones this does not
// reach. IPv6 stays blocked outside the tunnel: the driver guards one
// physical IPv6 address, and an interface can hold several.
//
class PermitNonTunnelIpv4 : public IFirewallRule
{
public:

	PermitNonTunnelIpv4() = default;
	~PermitNonTunnelIpv4() = default;

	bool apply(IObjectInstaller &objectInstaller) override;
};

}
