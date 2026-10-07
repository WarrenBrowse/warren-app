#pragma once

#include <winfw/rules/ifirewallrule.h>
#include <optional>
#include <string>
#include <vector>

namespace rules::applocks
{

//
// Apps locked to the VPN may use the tunnel interface and loopback, and the
// LAN when it is shared, and nothing else. The filters are persistent and
// belong to no policy, so they hold in every state, the disconnected one
// included, and while no daemon runs at all.
//
// Without a tunnel interface, or with one whose alias no longer resolves (the
// adapter was removed since), the locked apps get loopback (and the LAN)
// only: a lock never fails open.
//
// An app whose path cannot be resolved to an app id is left out, and with
// none left no filter is added at all: a filter with no app condition would
// match every app on the machine.
//
class LockToTunnel : public IFirewallRule
{
public:

	LockToTunnel(
		const std::vector<std::wstring> &apps,
		const std::optional<std::wstring> &tunnelInterfaceAlias,
		bool permitLan
	);

	~LockToTunnel() = default;

	bool apply(IObjectInstaller &objectInstaller) override;

private:

	bool applyBlock(IObjectInstaller &objectInstaller, const std::vector<std::wstring> &apps) const;
	bool applyPermitLan(IObjectInstaller &objectInstaller, const std::vector<std::wstring> &apps) const;

	const std::vector<std::wstring> m_apps;
	const std::optional<std::wstring> m_tunnelInterfaceAlias;
	const bool m_permitLan;
};

}
