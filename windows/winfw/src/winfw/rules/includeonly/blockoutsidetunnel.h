#pragma once

#include <winfw/rules/ifirewallrule.h>
#include <optional>
#include <string>
#include <vector>

namespace rules::includeonly
{

//
// "VPN only for these apps": the included apps may use the tunnel interface
// and loopback, and nothing else, in every policy.
//
// The split tunnel driver, engaged with the physical address as its "tunnel"
// address, soft-permits the apps it splits from every local address but that
// one, in every state where it holds addresses (the error and blocked states
// included). An included app binding a second interface's address, or an IPv6
// address other than the one the driver holds, would leave outside the tunnel
// past winfw's block-all. A hard block in a sublayer of its own outweighs any
// soft permit in another sublayer, the driver's included.
//
// Without a tunnel interface (blocked, error, and connecting before the
// interface exists) the included apps get loopback only.
//
// An app whose path cannot be resolved to an app id (not installed, or on a
// volume that is not mounted) cannot run, so it is left out. With none left
// no filter is added at all: a filter with no app condition would match every
// app on the machine.
//
class BlockOutsideTunnel : public IFirewallRule
{
public:

	BlockOutsideTunnel(
		const std::vector<std::wstring> &apps,
		const std::optional<std::wstring> &tunnelInterfaceAlias
	);

	~BlockOutsideTunnel() = default;

	bool apply(IObjectInstaller &objectInstaller) override;

private:

	const std::vector<std::wstring> m_apps;
	const std::optional<std::wstring> m_tunnelInterfaceAlias;
};

}
