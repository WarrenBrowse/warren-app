#include "stdafx.h"
#include "permittunnel.h"
#include <winfw/mullvadguids.h>
#include <winfw/rules/ports.h>
#include <winfw/rules/shared.h>
#include <libwfp/filterbuilder.h>
#include <libwfp/conditionbuilder.h>
#include <libwfp/conditions/conditionport.h>
#include <libwfp/conditions/conditionip.h>
#include <libwfp/conditions/conditioninterface.h>
#include <libcommon/error.h>

using namespace wfp::conditions;

namespace rules::dns
{

PermitTunnel::PermitTunnel(const std::wstring &tunnelInterfaceAlias, const std::vector<wfp::IpAddress> &hosts)
	: m_tunnelInterfaceAlias(tunnelInterfaceAlias)
	, m_anyHost(false)
{
	SplitAddresses(hosts, m_hostsIpv4, m_hostsIpv6);
}

PermitTunnel::PermitTunnel(const std::wstring &tunnelInterfaceAlias)
	: m_tunnelInterfaceAlias(tunnelInterfaceAlias)
	, m_anyHost(true)
{
}

//static
std::unique_ptr<PermitTunnel> PermitTunnel::AnyHost(const std::wstring &tunnelInterfaceAlias)
{
	return std::unique_ptr<PermitTunnel>(new PermitTunnel(tunnelInterfaceAlias));
}

bool PermitTunnel::apply(IObjectInstaller &objectInstaller)
{
	wfp::FilterBuilder filterBuilder;

	//
	// #1 Permit outbound DNS, IPv4.
	//

	if (m_anyHost || false == m_hostsIpv4.empty())
	{
		filterBuilder
			.key(MullvadGuids::Filter_Dns_PermitTunnel_Outbound_Ipv4())
			.name(m_anyHost
				? L"Permit DNS traffic to any resolver inside tunnel (IPv4)"
				: L"Permit selected DNS traffic inside tunnel (IPv4)")
			.description(L"This filter is part of a rule that permits DNS traffic inside the VPN tunnel")
			.provider(MullvadGuids::Provider())
			.layer(FWPM_LAYER_ALE_AUTH_CONNECT_V4)
			.sublayer(MullvadGuids::SublayerDns())
			.weight(wfp::FilterBuilder::WeightClass::Medium)
			.permit();

		wfp::ConditionBuilder conditionBuilder(FWPM_LAYER_ALE_AUTH_CONNECT_V4);

		conditionBuilder.add_condition(ConditionPort::Remote(DNS_SERVER_PORT));
		conditionBuilder.add_condition(ConditionInterface::Alias(m_tunnelInterfaceAlias));

		for (const auto &host : m_hostsIpv4)
		{
			conditionBuilder.add_condition(ConditionIp::Remote(host));
		}

		if (false == objectInstaller.addFilter(filterBuilder, conditionBuilder))
		{
			return false;
		}
	}

	if (false == m_anyHost && m_hostsIpv6.empty())
	{
		return true;
	}

	//
	// #2 Permit outbound DNS, IPv6.
	//

	filterBuilder
		.key(MullvadGuids::Filter_Dns_PermitTunnel_Outbound_Ipv6())
		.name(m_anyHost
				? L"Permit DNS traffic to any resolver inside tunnel (IPv6)"
				: L"Permit selected DNS traffic inside tunnel (IPv6)")
		.description(L"This filter is part of a rule that permits DNS traffic inside the VPN tunnel")
		.provider(MullvadGuids::Provider())
		.layer(FWPM_LAYER_ALE_AUTH_CONNECT_V6)
		.sublayer(MullvadGuids::SublayerDns())
		.weight(wfp::FilterBuilder::WeightClass::Medium)
		.permit();

	wfp::ConditionBuilder conditionBuilder(FWPM_LAYER_ALE_AUTH_CONNECT_V6);

	conditionBuilder.add_condition(ConditionPort::Remote(DNS_SERVER_PORT));
	conditionBuilder.add_condition(ConditionInterface::Alias(m_tunnelInterfaceAlias));

	for (const auto &host : m_hostsIpv6)
	{
		conditionBuilder.add_condition(ConditionIp::Remote(host));
	}

	return objectInstaller.addFilter(filterBuilder, conditionBuilder);
}

}
