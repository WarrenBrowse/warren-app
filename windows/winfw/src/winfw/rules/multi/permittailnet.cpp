#include "stdafx.h"
#include "permittailnet.h"
#include <winfw/mullvadguids.h>
#include <libwfp/filterbuilder.h>
#include <libwfp/conditionbuilder.h>
#include <libwfp/ipaddress.h>
#include <libwfp/ipnetwork.h>
#include <libwfp/conditions/conditioninterface.h>
#include <libwfp/conditions/conditionip.h>
#include <libwfp/conditions/conditionport.h>

using namespace wfp::conditions;

namespace rules::multi
{

namespace
{

const wfp::IpNetwork &TailnetV4()
{
	static const wfp::IpNetwork net(wfp::IpAddress(L"100.64.0.0"), 10);
	return net;
}

const wfp::IpNetwork &TailnetV6()
{
	static const wfp::IpNetwork net(wfp::IpAddress(L"fd7a:115c:a1e0::"), 48);
	return net;
}

constexpr uint16_t DNS_PORT = 53;

} // anonymous namespace

PermitTailnet::PermitTailnet(const std::wstring &interfaceAlias)
	: m_interfaceAlias(interfaceAlias)
{
}

bool PermitTailnet::apply(IObjectInstaller &objectInstaller)
{
	struct Leg
	{
		const GUID &layer;
		const wfp::IpNetwork &network;
		const wchar_t *name;
	};

	const Leg legs[] =
	{
		{ FWPM_LAYER_ALE_AUTH_CONNECT_V4, TailnetV4(), L"Permit outbound tailnet connections (IPv4)" },
		{ FWPM_LAYER_ALE_AUTH_CONNECT_V6, TailnetV6(), L"Permit outbound tailnet connections (IPv6)" },
		{ FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4, TailnetV4(), L"Permit inbound tailnet connections (IPv4)" },
		{ FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6, TailnetV6(), L"Permit inbound tailnet connections (IPv6)" },
	};

	//
	// No fixed keys: the rule is added once per Tailscale adapter.
	//
	for (const auto &leg : legs)
	{
		wfp::FilterBuilder filterBuilder(wfp::BuilderValidation::OnlyCritical);

		filterBuilder
			.name(leg.name)
			.description(L"This filter is part of a rule that permits traffic on a coexisting Tailscale adapter")
			.provider(MullvadGuids::Provider())
			.layer(leg.layer)
			.sublayer(MullvadGuids::SublayerBaseline())
			.weight(wfp::FilterBuilder::WeightClass::Medium)
			.permit();

		wfp::ConditionBuilder conditionBuilder(leg.layer);

		conditionBuilder.add_condition(ConditionInterface::Alias(m_interfaceAlias));
		conditionBuilder.add_condition(ConditionIp::Remote(leg.network));

		if (!objectInstaller.addFilter(filterBuilder, conditionBuilder))
		{
			return false;
		}
	}

	//
	// The DNS sublayer blocks port 53 everywhere but the tunnel's resolvers, and a
	// block in any sublayer wins: Tailscale's name service needs its own permit there.
	//
	const Leg dnsLegs[] =
	{
		{ FWPM_LAYER_ALE_AUTH_CONNECT_V4, TailnetV4(), L"Permit DNS to the tailnet (IPv4)" },
		{ FWPM_LAYER_ALE_AUTH_CONNECT_V6, TailnetV6(), L"Permit DNS to the tailnet (IPv6)" },
	};

	for (const auto &leg : dnsLegs)
	{
		wfp::FilterBuilder filterBuilder(wfp::BuilderValidation::OnlyCritical);

		filterBuilder
			.name(leg.name)
			.description(L"This filter is part of a rule that permits traffic on a coexisting Tailscale adapter")
			.provider(MullvadGuids::Provider())
			.layer(leg.layer)
			.sublayer(MullvadGuids::SublayerDns())
			.weight(wfp::FilterBuilder::WeightClass::Medium)
			.permit();

		wfp::ConditionBuilder conditionBuilder(leg.layer);

		conditionBuilder.add_condition(ConditionInterface::Alias(m_interfaceAlias));
		conditionBuilder.add_condition(ConditionIp::Remote(leg.network));
		conditionBuilder.add_condition(ConditionPort::Remote(DNS_PORT));

		if (!objectInstaller.addFilter(filterBuilder, conditionBuilder))
		{
			return false;
		}
	}

	return true;
}

}
