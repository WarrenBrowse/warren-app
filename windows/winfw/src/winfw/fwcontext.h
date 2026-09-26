#pragma once

#include "winfw.h"
#include "sessioncontroller.h"
#include "rules/ifirewallrule.h"
#include "libwfp/ipaddress.h"
#include <cstdint>
#include <memory>
#include <vector>
#include <string>
#include <optional>

class FwContext
{
public:

	FwContext(uint32_t timeout);

	// This ctor applies the "blocked" policy.
	FwContext
	(
		uint32_t timeout,
		const WinFwSettings &settings,
		const std::optional<WinFwAllowedEndpoint> &allowedEndpoint
	);

	bool applyPolicyConnecting
	(
		const WinFwSettings &settings,
		const std::vector<WinFwEndpoint> &relays,
		const std::optional<wfp::IpAddress> &exitEndpointIp,
		const std::vector<std::wstring> &relayClients,
		const std::optional<std::wstring> &tunnelInterfaceAlias,
		const std::optional<WinFwAllowedEndpoint> &allowedEndpoint,
		const WinFwAllowedTunnelTraffic &allowedTunnelTraffic
	);

	bool applyPolicyConnected
	(
		const WinFwSettings &settings,
		const std::vector<WinFwEndpoint> &relays,
		const std::optional<wfp::IpAddress> &exitEndpointIp,
		const std::vector<std::wstring> &relayClients,
		const std::wstring &tunnelInterfaceAlias,
		const std::vector<wfp::IpAddress> &tunnelDnsServers,
		const std::vector<wfp::IpAddress> &nonTunnelDnsServers,
		bool allowExternalDns
	);

	bool applyPolicyBlocked(
		const WinFwSettings &settings,
		const std::optional<WinFwAllowedEndpoint> &allowedEndpoint
	);

	bool reset();

	//
	// "VPN only for these apps": holds `apps` to the tunnel interface and
	// loopback in every policy from now on (an empty list lifts it), and
	// re-applies the active policy at once so no state runs without it.
	//
	bool setIncludedApps(const std::vector<std::wstring> &apps);

	enum class Policy
	{
		Connecting,
		Connected,
		Blocked,
		None,
	};

	Policy activePolicy() const;

	using Ruleset = std::vector<std::unique_ptr<rules::IFirewallRule> >;

private:

	FwContext(const FwContext &) = delete;
	FwContext &operator=(const FwContext &) = delete;

	Ruleset composePolicyBlocked(const WinFwSettings &settings, const std::optional<WinFwAllowedEndpoint> &allowedEndpoint);

	bool applyBaseConfiguration();
	bool applyBlockedBaseConfiguration(const WinFwSettings &settings, const std::optional<WinFwAllowedEndpoint> &allowedEndpoint, uint32_t &checkpoint);
	bool applyCommonBaseConfiguration(SessionController &controller, wfp::FilterEngine &engine);

	bool applyPolicy(Ruleset &&ruleset, const std::optional<std::wstring> &tunnelInterfaceAlias, Policy policy);
	bool applyRulesetDirectly(const Ruleset &ruleset, SessionController &controller);
	Ruleset composeIncludeOnlyGuard(const std::optional<std::wstring> &tunnelInterfaceAlias) const;

	std::unique_ptr<SessionController> m_sessionController;

	uint32_t m_baseline;
	Policy m_activePolicy;

	//
	// The active policy's rules and tunnel interface, kept so a change of the
	// included apps can re-apply the policy with a new guard.
	//
	Ruleset m_activeRuleset;
	std::optional<std::wstring> m_activeTunnelInterfaceAlias;

	std::vector<std::wstring> m_includedApps;
};
