#include "stdafx.h"
#include "permitnontunnelipv4.h"
#include <winfw/mullvadguids.h>
#include <libwfp/filterbuilder.h>
#include <libwfp/nullconditionbuilder.h>

namespace rules::baseline
{

bool PermitNonTunnelIpv4::apply(IObjectInstaller &objectInstaller)
{
	wfp::FilterBuilder filterBuilder;
	wfp::NullConditionBuilder nullConditionBuilder;

	//
	// #1 Permit outbound connections, IPv4.
	//

	filterBuilder
		.key(MullvadGuids::Filter_Baseline_PermitNonTunnel_Outbound_Ipv4())
		.name(L"Permit outbound connections outside the tunnel (IPv4)")
		.description(L"This filter is part of a rule that lets apps outside \"VPN only for these apps\" use the normal connection")
		.provider(MullvadGuids::Provider())
		.layer(FWPM_LAYER_ALE_AUTH_CONNECT_V4)
		.sublayer(MullvadGuids::SublayerBaseline())
		.weight(wfp::FilterBuilder::WeightClass::Medium)
		.permit();

	if (false == objectInstaller.addFilter(filterBuilder, nullConditionBuilder))
	{
		return false;
	}

	//
	// #2 Permit inbound connections, IPv4.
	//

	filterBuilder
		.key(MullvadGuids::Filter_Baseline_PermitNonTunnel_Inbound_Ipv4())
		.name(L"Permit inbound connections outside the tunnel (IPv4)")
		.layer(FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4);

	return objectInstaller.addFilter(filterBuilder, nullConditionBuilder);
}

}
