#pragma once

#include <winfw/rules/ifirewallrule.h>
#include <string>

namespace rules::includeonly
{

//
// "VPN only for these apps": the connected policy lets IPv4 out of the
// tunnel for every app the driver does not hold, the system resolver included.
// Port 53 stays confined to the tunnel resolvers, but DNS over HTTPS or TLS
// to a resolver configured on a physical adapter would carry the names an
// included app looks up from the physical address. This blocks the account
// that resolver runs as (the Dnscache service SID) on ports 443 and 853 off
// the tunnel interface.
//
class BlockSystemResolverOffTunnel : public IFirewallRule
{
public:

	//
	// `account` is the resolver's account, `NT SERVICE\Dnscache` in the
	// policy; a parameter so it can be tested against an account a test owns.
	//
	BlockSystemResolverOffTunnel(const std::wstring &account, const std::wstring &tunnelInterfaceAlias);

	~BlockSystemResolverOffTunnel() = default;

	bool apply(IObjectInstaller &objectInstaller) override;

	static const wchar_t *SystemResolverAccount();

private:

	const std::wstring m_account;
	const std::wstring m_tunnelInterfaceAlias;
};

}
