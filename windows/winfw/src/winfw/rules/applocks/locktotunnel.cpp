#include "stdafx.h"
#include "locktotunnel.h"
#include <winfw/mullvadguids.h>
#include <winfw/lannetworks.h>
#include <winfw/rules/includeonly/blockoutsidetunnel.h>
#include <libwfp/filterbuilder.h>
#include <libwfp/conditionbuilder.h>
#include <libwfp/conditions/comparison.h>
#include <libwfp/conditions/conditioninterface.h>
#include <libwfp/conditions/conditionip.h>
#include <libwfp/conditions/conditionloopback.h>
#include <array>
#include <memory>

using namespace wfp::conditions;

namespace rules::applocks
{

namespace
{

void AddApps(wfp::ConditionBuilder &conditionBuilder, const std::vector<std::wstring> &apps)
{
	for (const auto &app : apps)
	{
		conditionBuilder.add_condition(includeonly::AppCondition(app));
	}
}

//
// The tunnel interface named by `alias`, or none when the alias no longer
// resolves: the adapter may have gone away since the daemon last saw it, and
// the lock must then block outside loopback rather than fail to install.
//
std::unique_ptr<ConditionInterface> NotTunnelInterface(const std::optional<std::wstring> &alias)
{
	if (false == alias.has_value())
	{
		return nullptr;
	}

	try
	{
		return ConditionInterface::Alias(*alias, CompareNeq());
	}
	catch (...)
	{
		return nullptr;
	}
}

} // anonymous namespace

LockToTunnel::LockToTunnel(
	const std::vector<std::wstring> &apps,
	const std::optional<std::wstring> &tunnelInterfaceAlias,
	bool permitLan
)
	: m_apps(apps)
	, m_tunnelInterfaceAlias(tunnelInterfaceAlias)
	, m_permitLan(permitLan)
{
}

bool LockToTunnel::apply(IObjectInstaller &objectInstaller)
{
	const auto apps = includeonly::ResolvableApps(m_apps);

	if (apps.empty())
	{
		return true;
	}

	if (false == applyBlock(objectInstaller, apps))
	{
		return false;
	}

	return false == m_permitLan || applyPermitLan(objectInstaller, apps);
}

bool LockToTunnel::applyBlock(IObjectInstaller &objectInstaller, const std::vector<std::wstring> &apps) const
{
	const std::array<const GUID *, 4> layers =
	{
		&FWPM_LAYER_ALE_AUTH_CONNECT_V4,
		&FWPM_LAYER_ALE_AUTH_CONNECT_V6,
		&FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
		&FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
	};

	for (const auto layer : layers)
	{
		wfp::FilterBuilder filterBuilder(wfp::BuilderValidation::OnlyCritical);

		//
		// Hard (definitive) so that no permit in another sublayer, the split
		// tunnel driver's included, can outweigh it.
		//
		filterBuilder
			.name(L"Block the apps locked to the VPN outside the tunnel")
			.description(L"This filter is part of a rule that holds the apps locked to the VPN to the tunnel and loopback")
			.provider(MullvadGuids::ProviderAppLocks())
			.layer(*layer)
			.sublayer(MullvadGuids::SublayerAppLocks())
			.weight(wfp::FilterBuilder::WeightClass::Medium)
			.persistent()
			.definitive()
			.block();

		wfp::ConditionBuilder conditionBuilder(*layer);

		AddApps(conditionBuilder, apps);

		conditionBuilder.add_condition(std::make_unique<ConditionLoopback>(
			ConditionLoopback::Type::LoopbackTraffic, CompareNeq()));

		if (auto notTunnel = NotTunnelInterface(m_tunnelInterfaceAlias))
		{
			conditionBuilder.add_condition(std::move(notTunnel));
		}

		if (false == objectInstaller.addFilter(filterBuilder, conditionBuilder))
		{
			return false;
		}
	}

	return true;
}

//
// Weighed above the block in the same sublayer, so it decides for LAN
// destinations there; any other sublayer still decides for itself, which
// keeps a connected policy without LAN sharing in force.
//
bool LockToTunnel::applyPermitLan(IObjectInstaller &objectInstaller, const std::vector<std::wstring> &apps) const
{
	struct LanLayer
	{
		const GUID *layer;
		bool ipv6;
		bool outbound;
	};

	const std::array<LanLayer, 4> layers =
	{
		LanLayer{ &FWPM_LAYER_ALE_AUTH_CONNECT_V4, false, true },
		LanLayer{ &FWPM_LAYER_ALE_AUTH_CONNECT_V6, true, true },
		LanLayer{ &FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4, false, false },
		LanLayer{ &FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6, true, false },
	};

	for (const auto &lanLayer : layers)
	{
		wfp::FilterBuilder filterBuilder(wfp::BuilderValidation::OnlyCritical);

		filterBuilder
			.name(L"Permit the apps locked to the VPN on the LAN")
			.description(L"This filter is part of a rule that lets the apps locked to the VPN share the LAN")
			.provider(MullvadGuids::ProviderAppLocks())
			.layer(*lanLayer.layer)
			.sublayer(MullvadGuids::SublayerAppLocks())
			.weight(wfp::FilterBuilder::WeightClass::Max)
			.persistent()
			.permit();

		wfp::ConditionBuilder conditionBuilder(*lanLayer.layer);

		AddApps(conditionBuilder, apps);

		if (lanLayer.ipv6)
		{
			for (const auto &network : g_ipv6LanNets)
			{
				conditionBuilder.add_condition(ConditionIp::Remote(network));
			}

			if (lanLayer.outbound)
			{
				for (const auto &network : g_ipv6MulticastNets)
				{
					conditionBuilder.add_condition(ConditionIp::Remote(network));
				}
			}
		}
		else
		{
			for (const auto &network : g_ipv4LanNets)
			{
				conditionBuilder.add_condition(ConditionIp::Remote(network));
			}

			if (lanLayer.outbound)
			{
				for (const auto &network : g_ipv4MulticastNets)
				{
					conditionBuilder.add_condition(ConditionIp::Remote(network));
				}
			}
		}

		if (false == objectInstaller.addFilter(filterBuilder, conditionBuilder))
		{
			return false;
		}
	}

	return true;
}

}
