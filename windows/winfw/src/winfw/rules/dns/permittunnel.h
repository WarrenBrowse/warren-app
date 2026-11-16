#pragma once

#include <winfw/rules/ifirewallrule.h>
#include <libwfp/ipaddress.h>
#include <vector>
#include <string>
#include <memory>

namespace rules::dns
{

class PermitTunnel : public IFirewallRule
{
public:

	PermitTunnel(const std::wstring &tunnelInterfaceAlias, const std::vector<wfp::IpAddress> &hosts);

	// Port 53 to any host, as long as it leaves through the tunnel interface.
	static std::unique_ptr<PermitTunnel> AnyHost(const std::wstring &tunnelInterfaceAlias);

	bool apply(IObjectInstaller &objectInstaller) override;

private:

	explicit PermitTunnel(const std::wstring &tunnelInterfaceAlias);

	const std::wstring m_tunnelInterfaceAlias;
	const bool m_anyHost;
	std::vector<wfp::IpAddress> m_hostsIpv4;
	std::vector<wfp::IpAddress> m_hostsIpv6;
};

}
