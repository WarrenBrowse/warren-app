#pragma once

#include <winfw/rules/ifirewallrule.h>
#include <string>

namespace rules::multi
{

//
// Permits tailnet traffic on a Tailscale adapter that coexists with the tunnel,
// and nothing else there: out towards, and in from, Tailscale's address ranges,
// DNS towards them included (Tailscale's name service answers inside that
// adapter). Whatever enters the adapter is encrypted by Tailscale, whose own
// sockets the rest of the policy governs.
//
class PermitTailnet : public IFirewallRule
{
public:

	explicit PermitTailnet(const std::wstring &interfaceAlias);

	bool apply(IObjectInstaller &objectInstaller) override;

private:

	const std::wstring m_interfaceAlias;
};

}
