#include "stdafx.h"
#include "fwcontext.h"
#include "mullvadobjects.h"
#include "objectpurger.h"
#include "sharedsublayers.h"
#include "rules/ifirewallrule.h"
#include "rules/ports.h"
#include "rules/baseline/blockall.h"
#include "rules/baseline/permitdhcp.h"
#include "rules/baseline/permitndp.h"
#include "rules/baseline/permitdhcpserver.h"
#include "rules/baseline/permitlan.h"
#include "rules/baseline/permitlanservice.h"
#include "rules/baseline/permitnontunnelipv4.h"
#include "rules/baseline/permitloopback.h"
#include "rules/baseline/permitvpntunnel.h"
#include "rules/baseline/permitvpntunnelservice.h"
#include "rules/baseline/permitdns.h"
#include "rules/dns/blockall.h"
#include "rules/dns/permitloopback.h"
#include "rules/dns/permittunnel.h"
#include "rules/dns/permitnontunnel.h"
#include "rules/multi/permitendpoint.h"
#include "rules/includeonly/blockoutsidetunnel.h"
#include "rules/includeonly/blocksystemresolver.h"
#include <libwfp/transaction.h>
#include <libwfp/filterengine.h>
#include <libcommon/error.h>
#include <functional>
#include <utility>

using namespace rules;

namespace
{

//
// Since the PermitLan rule doesn't specifically address DNS, it will allow DNS requests targeting
// a local resolver to leave the machine. From the local resolver the request will either be
// resolved from cache, or forwarded out onto the Internet.
//
// Therefore, we're unconditionally lifting all DNS traffic out of the baseline sublayer and restricting
// it in the DNS sublayer instead. The PermitDNS rule in the baseline sublayer accomplishes this.
//
// This has implications for the way the relay access is configured. In the regular case there
// is no issue: The PermitEndpoint rule can be installed in the baseline sublayer.
//
// However, if the relay is running on the DNS port (53), it would be blocked unless the DNS
// sublayer permits this traffic. For this reason, whenever the relay is on port 53, the
// PermitEndpoint rule has to be installed to the DNS sublayer instead of the baseline sublayer.
//
void AppendSettingsRules
(
	FwContext::Ruleset &ruleset,
	const WinFwSettings &settings,
	bool allowExternalDns = false
)
{
	if (settings.permitDhcp)
	{
		ruleset.emplace_back(std::make_unique<baseline::PermitDhcp>());
		ruleset.emplace_back(std::make_unique<baseline::PermitNdp>());
	}

	if (settings.permitLan)
	{
		ruleset.emplace_back(std::make_unique<baseline::PermitLan>());
		ruleset.emplace_back(std::make_unique<baseline::PermitLanService>());
		ruleset.emplace_back(baseline::PermitDhcpServer::WithExtent(baseline::PermitDhcpServer::Extent::IPv4Only));
	}

	//
	// DNS management
	//

	ruleset.emplace_back(std::make_unique<baseline::PermitDns>());
	ruleset.emplace_back(std::make_unique<dns::PermitLoopback>());

	// The block-all DNS rule is what enforces DNS leak protection. The advanced `allowExternalDns`
	// opt-in skips it so queries to arbitrary resolvers are permitted (they still go through the
	// tunnel). Without this rule, DNS falls through to the baseline sublayer and is permitted by the
	// PermitVpnTunnel rule on the tunnel interface.
	if (!allowExternalDns)
	{
		ruleset.emplace_back(std::make_unique<dns::BlockAll>());
	}
}

//
// Refer comment on `AppendSettingsRules`.
//
void AppendRelayRules
(
	FwContext::Ruleset &ruleset,
	const WinFwEndpoint &relay,
	const std::vector<std::wstring> &relayClients
)
{
	auto sublayer =
	(
		DNS_SERVER_PORT == relay.port
		? rules::multi::PermitEndpoint::Sublayer::Dns
		: rules::multi::PermitEndpoint::Sublayer::Baseline
	);

	ruleset.emplace_back(std::make_unique<multi::PermitEndpoint>(
		wfp::IpAddress(relay.ip),
		relay.port,
		relay.protocol,
		relayClients,
		sublayer
	));
}

//
// Refer comment on `AppendSettingsRules`.
//
void AppendAllowedEndpointRules
(
	FwContext::Ruleset &ruleset,
	const WinFwAllowedEndpoint &endpoint
)
{
	std::vector<std::wstring> clients;
	clients.reserve(endpoint.numClients);
	for (uint32_t i = 0; i < endpoint.numClients; i++) {
		clients.push_back(endpoint.clients[i]);
	}

	auto sublayer =
	(
		DNS_SERVER_PORT == endpoint.endpoint.port
		? rules::multi::PermitEndpoint::Sublayer::Dns
		: rules::multi::PermitEndpoint::Sublayer::Baseline
	);

	ruleset.emplace_back(std::make_unique<multi::PermitEndpoint>(
		wfp::IpAddress(endpoint.endpoint.ip),
		endpoint.endpoint.port,
		endpoint.endpoint.protocol,
		clients,
		sublayer
	));
}

void AppendNetBlockedRules(FwContext::Ruleset &ruleset)
{
	ruleset.emplace_back(std::make_unique<baseline::BlockAll>());
	ruleset.emplace_back(std::make_unique<baseline::PermitLoopback>());
}

} // anonymous namespace

FwContext::FwContext
(
	uint32_t timeout
)
	: m_baseline(0)
	, m_activePolicy(Policy::None)
{
	auto engine = wfp::FilterEngine::StandardSession(timeout);

	//
	// Pass engine ownership to "session controller"
	//
	m_sessionController = std::make_unique<SessionController>(std::move(engine));

	if (false == applyBaseConfiguration())
	{
		THROW_ERROR("Failed to apply base configuration in BFE");
	}

	m_baseline = m_sessionController->checkpoint();
	m_activePolicy = Policy::None;
}

FwContext::FwContext
(
	uint32_t timeout,
	const WinFwSettings &settings,
	const std::optional<WinFwAllowedEndpoint> &allowedEndpoint
)
	: m_baseline(0)
	, m_activePolicy(Policy::None)
{
	auto engine = wfp::FilterEngine::StandardSession(timeout);

	//
	// Pass engine ownership to "session controller"
	//
	m_sessionController = std::make_unique<SessionController>(std::move(engine));

	uint32_t checkpoint = 0;

	if (false == applyBlockedBaseConfiguration(settings, allowedEndpoint, checkpoint))
	{
		THROW_ERROR("Failed to apply base configuration in BFE");
	}

	m_baseline = checkpoint;
	m_activePolicy = Policy::Blocked;
}

bool FwContext::applyPolicyConnecting
(
	const WinFwSettings &settings,
	const std::vector<WinFwEndpoint> &relays,
	const std::optional<wfp::IpAddress> &exitEndpointIp,
	const std::vector<std::wstring> &relayClients,
	const std::optional<std::wstring> &tunnelInterfaceAlias,
	const std::optional<WinFwAllowedEndpoint> &allowedEndpoint,
	const WinFwAllowedTunnelTraffic &allowedTunnelTraffic
)
{
	Ruleset ruleset;

	AppendNetBlockedRules(ruleset);
	AppendSettingsRules(ruleset, settings);

	for (const auto &relay : relays)
	{
		AppendRelayRules(ruleset, relay, relayClients);
	}

	if (allowedEndpoint.has_value())
	{
		AppendAllowedEndpointRules(ruleset, allowedEndpoint.value());
	}

	if (tunnelInterfaceAlias.has_value())
	{
		switch (allowedTunnelTraffic.type)
		{
			case WinFwAllowedTunnelTrafficType::All:
			{
				ruleset.emplace_back(std::make_unique<baseline::PermitVpnTunnel>(
					relayClients,
					*tunnelInterfaceAlias,
					std::nullopt,
					exitEndpointIp
				));
				ruleset.emplace_back(std::make_unique<baseline::PermitVpnTunnelService>(
					relayClients,
					*tunnelInterfaceAlias,
					std::nullopt,
					exitEndpointIp
				));
				break;
			}
			case WinFwAllowedTunnelTrafficType::One:
			{
				auto onlyEndpoint = std::make_optional<baseline::PermitVpnTunnel::Endpoints>({
						baseline::PermitVpnTunnel::Endpoint{
						wfp::IpAddress(allowedTunnelTraffic.endpoint1->ip),
						allowedTunnelTraffic.endpoint1->port,
						allowedTunnelTraffic.endpoint1->protocol
						},
						std::nullopt,
				});
				ruleset.emplace_back(std::make_unique<baseline::PermitVpnTunnel>(
					relayClients,
					*tunnelInterfaceAlias,
					onlyEndpoint,
					exitEndpointIp
				));
				ruleset.emplace_back(std::make_unique<baseline::PermitVpnTunnelService>(
					relayClients,
					*tunnelInterfaceAlias,
					onlyEndpoint,
					exitEndpointIp
				));
				break;
			}
			case WinFwAllowedTunnelTrafficType::Two:
			{
				auto endpoints = std::make_optional<baseline::PermitVpnTunnel::Endpoints>({
						baseline::PermitVpnTunnel::Endpoint{
						wfp::IpAddress(allowedTunnelTraffic.endpoint1->ip),
						allowedTunnelTraffic.endpoint1->port,
						allowedTunnelTraffic.endpoint1->protocol
						},
						std::make_optional<baseline::PermitVpnTunnel::Endpoint>({
								wfp::IpAddress(allowedTunnelTraffic.endpoint2->ip),
								allowedTunnelTraffic.endpoint2->port,
								allowedTunnelTraffic.endpoint2->protocol
								})
				});
				ruleset.emplace_back(std::make_unique<baseline::PermitVpnTunnel>(
							relayClients,
							*tunnelInterfaceAlias,
							endpoints,
							exitEndpointIp
							));
				ruleset.emplace_back(std::make_unique<baseline::PermitVpnTunnelService>(
							relayClients,
							*tunnelInterfaceAlias,
							endpoints,
							exitEndpointIp
							));
				break;
			}
			// For the "None" case, do nothing.
		}
	}

	return applyPolicy(std::move(ruleset), tunnelInterfaceAlias, Policy::Connecting);
}

bool FwContext::applyPolicyConnected
(
	const WinFwSettings &settings,
	const std::vector<WinFwEndpoint> &relays,
	const std::optional<wfp::IpAddress> &exitEndpointIp,
	const std::vector<std::wstring> &relayClients,
	const std::wstring &tunnelInterfaceAlias,
	const std::vector<wfp::IpAddress> &tunnelDnsServers,
	const std::vector<wfp::IpAddress> &nonTunnelDnsServers,
	bool allowExternalDns
)
{
	Ruleset ruleset;

	AppendNetBlockedRules(ruleset);
	AppendSettingsRules(ruleset, settings, allowExternalDns);

	for (const auto &relay : relays)
	{
		AppendRelayRules(ruleset, relay, relayClients);
	}

	if (!tunnelDnsServers.empty())
	{
		ruleset.emplace_back(std::make_unique<dns::PermitTunnel>(
			tunnelInterfaceAlias, tunnelDnsServers
		));
	}
	if (!nonTunnelDnsServers.empty())
	{
		ruleset.emplace_back(std::make_unique<dns::PermitNonTunnel>(
			tunnelInterfaceAlias, nonTunnelDnsServers
		));
	}

	ruleset.emplace_back(std::make_unique<baseline::PermitVpnTunnel>(
		relayClients,
		tunnelInterfaceAlias,
		std::nullopt,
		exitEndpointIp
	));

	ruleset.emplace_back(std::make_unique<baseline::PermitVpnTunnelService>(
		relayClients,
		tunnelInterfaceAlias,
		std::nullopt,
		exitEndpointIp
	));

	if (settings.permitNonTunnelIpv4)
	{
		ruleset.emplace_back(std::make_unique<baseline::PermitNonTunnelIpv4>());
		ruleset.emplace_back(std::make_unique<includeonly::BlockSystemResolverOffTunnel>(
			includeonly::BlockSystemResolverOffTunnel::SystemResolverAccount(),
			tunnelInterfaceAlias
		));
	}

	return applyPolicy(std::move(ruleset), tunnelInterfaceAlias, Policy::Connected);
}

bool FwContext::applyPolicyBlocked(const WinFwSettings &settings, const std::optional<WinFwAllowedEndpoint> &allowedEndpoint)
{
	return applyPolicy(composePolicyBlocked(settings, allowedEndpoint), std::nullopt, Policy::Blocked);
}

bool FwContext::reset()
{
	const auto status = m_sessionController->executeTransaction([this](SessionController &controller, wfp::FilterEngine &)
	{
		return controller.revert(m_baseline), true;
	});

	if (status)
	{
		m_activePolicy = Policy::None;
		m_activeRuleset.clear();
		m_activeTunnelInterfaceAlias.reset();
	}

	return status;
}

bool FwContext::setIncludedApps(const std::vector<std::wstring> &apps)
{
	if (Policy::None == m_activePolicy)
	{
		m_includedApps = apps;
		return true;
	}

	auto previous = std::move(m_includedApps);
	m_includedApps = apps;

	const auto guard = composeIncludeOnlyGuard(m_activeTunnelInterfaceAlias);

	const auto status = m_sessionController->executeTransaction([&](SessionController &controller, wfp::FilterEngine &)
	{
		controller.revert(m_baseline);
		return applyRulesetDirectly(m_activeRuleset, controller)
			&& applyRulesetDirectly(guard, controller);
	});

	if (false == status)
	{
		m_includedApps = std::move(previous);
	}

	return status;
}

FwContext::Policy FwContext::activePolicy() const
{
	return m_activePolicy;
}

FwContext::Ruleset FwContext::composePolicyBlocked(const WinFwSettings &settings, const std::optional<WinFwAllowedEndpoint> &allowedEndpoint)
{
	Ruleset ruleset;

	AppendNetBlockedRules(ruleset);
	AppendSettingsRules(ruleset, settings);

	if (allowedEndpoint.has_value())
	{
		AppendAllowedEndpointRules(ruleset, allowedEndpoint.value());
	}

	return ruleset;
}

bool FwContext::applyBaseConfiguration()
{
	return m_sessionController->executeTransaction([this](SessionController &controller, wfp::FilterEngine &engine)
	{
		return applyCommonBaseConfiguration(controller, engine);
	});
}

bool FwContext::applyBlockedBaseConfiguration(const WinFwSettings &settings, const std::optional<WinFwAllowedEndpoint> &allowedEndpoint, uint32_t &checkpoint)
{
	return m_sessionController->executeTransaction([&](SessionController &controller, wfp::FilterEngine &engine)
	{
		if (false == applyCommonBaseConfiguration(controller, engine))
		{
			return false;
		}

		//
		// Record the current session state with only structural objects added.
		// If we snapshot at a later time we'd accidentally include the blocking policy rules
		// in the baseline checkpoint.
		//
		checkpoint = controller.peekCheckpoint();

		m_activeRuleset = composePolicyBlocked(settings, allowedEndpoint);

		return applyRulesetDirectly(m_activeRuleset, controller);
	});
}

bool FwContext::applyCommonBaseConfiguration(SessionController &controller, wfp::FilterEngine &engine)
{
	//
	// Since we're using a standard WFP session we can make no assumptions
	// about which objects are already installed since before.
	//
	ObjectPurger::GetRemoveAllFunctor()(engine);

	const auto share = shared_sublayers::MayAdopt(engine, MullvadGuids::Provider());

	MullvadGuids::UseSharedSublayers(share);

	//
	// Install structural objects. The shared sublayers belong to no provider,
	// so they are installed outside the session's journal, which only ever
	// reverts what it can own.
	//
	if (false == controller.addProvider(*MullvadObjects::Provider()))
	{
		return false;
	}

	if (share)
	{
		shared_sublayers::Install(engine);

		if (false == shared_sublayers::Claim(controller))
		{
			return false;
		}
	}
	else if (false == controller.addSublayer(*MullvadObjects::SublayerBaseline())
		|| false == controller.addSublayer(*MullvadObjects::SublayerDns()))
	{
		return false;
	}

	return controller.addSublayer(*MullvadObjects::SublayerIncludeOnly());
}

bool FwContext::applyPolicy(Ruleset &&ruleset, const std::optional<std::wstring> &tunnelInterfaceAlias, Policy policy)
{
	const auto guard = composeIncludeOnlyGuard(tunnelInterfaceAlias);

	const auto status = m_sessionController->executeTransaction([&](SessionController &controller, wfp::FilterEngine &)
	{
		controller.revert(m_baseline);
		return applyRulesetDirectly(ruleset, controller)
			&& applyRulesetDirectly(guard, controller);
	});

	if (status)
	{
		m_activeRuleset = std::move(ruleset);
		m_activeTunnelInterfaceAlias = tunnelInterfaceAlias;
		m_activePolicy = policy;
	}

	return status;
}

FwContext::Ruleset FwContext::composeIncludeOnlyGuard(const std::optional<std::wstring> &tunnelInterfaceAlias) const
{
	Ruleset guard;

	if (false == m_includedApps.empty())
	{
		guard.emplace_back(std::make_unique<includeonly::BlockOutsideTunnel>(
			m_includedApps, tunnelInterfaceAlias));
	}

	return guard;
}

bool FwContext::applyRulesetDirectly(const Ruleset &ruleset, SessionController &controller)
{
	for (const auto &rule : ruleset)
	{
		if (false == rule->apply(controller))
		{
			return false;
		}
	}

	return true;
}
