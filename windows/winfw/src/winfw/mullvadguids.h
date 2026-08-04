#pragma once

#include "guidhash.h"
#include <guiddef.h>
#include <cstdint>
#include <unordered_set>
#include <map>

//
// Every WFP object this app installs is keyed by a GUID, and the purge removes
// every object carrying our provider key. Those keys live in one machine-wide
// namespace, so a non-production build sharing them would tear down a
// production install's kill-switch (and vice versa). Each environment derives
// its own key set from a build-time salt instead.
//
// Production's salt is 0, so its GUIDs stay bit for bit what they have always
// been: an upgrade keeps recognising the objects it installed before, and the
// change cannot affect production at all. The salt is injected by the native
// module build from WARREN_PRODUCT_ENV.
//
#ifndef WARREN_FW_GUID_SALT
#define WARREN_FW_GUID_SALT 0ul
#endif

constexpr GUID WarrenEnvGuid(GUID g)
{
	g.Data1 ^= static_cast<unsigned long>(WARREN_FW_GUID_SALT);
	return g;
}

//
// Rekeys one of our object keys to another environment's salt.
//
// Needed by recovery only. A kill switch outlives the process that armed it,
// so a machine can carry blocking objects keyed for an environment it no
// longer runs (an install that moved from one channel to another, or a build
// that predates the salt). Those objects answer to no current build: the
// running one purges its own key set and never sees them, which leaves the
// machine blocked with no way out. Recovery therefore sweeps every known salt.
//
// This is deliberately NOT used on the normal startup purge: outside recovery,
// deleting another environment's objects would disarm a kill switch that
// environment is actively relying on.
//
constexpr GUID WarrenGuidForSalt(GUID g, uint32_t salt)
{
	g.Data1 ^= static_cast<unsigned long>(WARREN_FW_GUID_SALT) ^ static_cast<unsigned long>(salt);
	return g;
}

class MullvadGuids
{
public:

	MullvadGuids() = delete;

	static const GUID &Provider();
	static const GUID &SublayerBaseline();
	static const GUID &SublayerDns();

	//
	// Filter identifiers
	// Naming convention: Filter_sublayer_rule_filter
	//

	static const GUID &Filter_Baseline_BlockAll_Outbound_Ipv4();
	static const GUID &Filter_Baseline_BlockAll_Inbound_Ipv4();
	static const GUID &Filter_Baseline_BlockAll_Outbound_Ipv6();
	static const GUID &Filter_Baseline_BlockAll_Inbound_Ipv6();

	static const GUID &Filter_Baseline_PermitLan_Outbound_Ipv4();
	static const GUID &Filter_Baseline_PermitLan_Outbound_Multicast_Ipv4();
	static const GUID &Filter_Baseline_PermitLan_Outbound_Ipv6();
	static const GUID &Filter_Baseline_PermitLan_Outbound_Multicast_Ipv6();

	static const GUID &Filter_Baseline_PermitLanService_Inbound_Ipv4();
	static const GUID &Filter_Baseline_PermitLanService_Inbound_Ipv6();

	static const GUID &Filter_Baseline_PermitLoopback_Outbound_Ipv4();
	static const GUID &Filter_Baseline_PermitLoopback_Inbound_Ipv4();
	static const GUID &Filter_Baseline_PermitLoopback_Outbound_Ipv6();
	static const GUID &Filter_Baseline_PermitLoopback_Inbound_Ipv6();

	static const GUID &Filter_Baseline_PermitDhcp_Outbound_Request_Ipv4();
	static const GUID &Filter_Baseline_PermitDhcp_Inbound_Response_Ipv4();
	static const GUID &Filter_Baseline_PermitDhcp_Outbound_Request_Ipv6();
	static const GUID &Filter_Baseline_PermitDhcp_Inbound_Response_Ipv6();

	static const GUID &Filter_Baseline_PermitDhcpServer_Inbound_Request_Ipv4();
	static const GUID &Filter_Baseline_PermitDhcpServer_Outbound_Response_Ipv4();

	static const GUID &Filter_Baseline_PermitVpnTunnel_Outbound_Ipv4_1();
	static const GUID &Filter_Baseline_PermitVpnTunnel_Outbound_Ipv6_1();
	static const GUID &Filter_Baseline_PermitVpnTunnel_Outbound_Ipv4_2();
	static const GUID &Filter_Baseline_PermitVpnTunnel_Outbound_Ipv6_2();
	static const GUID &Filter_Baseline_PermitVpnTunnel_ExitIp();
	static const GUID &Filter_Baseline_PermitVpnTunnel_BlockExitIp();

	static const GUID &Filter_Baseline_PermitVpnTunnelService_Ipv4_1();
	static const GUID &Filter_Baseline_PermitVpnTunnelService_Ipv6_1();
	static const GUID &Filter_Baseline_PermitVpnTunnelService_Ipv4_2();
	static const GUID &Filter_Baseline_PermitVpnTunnelService_Ipv6_2();
	static const GUID &Filter_Baseline_PermitVpnTunnelService_ExitIp();
	static const GUID &Filter_Baseline_PermitVpnTunnelService_BlockExitIp();

	static const GUID &Filter_Baseline_PermitNdp_Outbound_Router_Solicitation();
	static const GUID &Filter_Baseline_PermitNdp_Inbound_Router_Advertisement();
	static const GUID &Filter_Baseline_PermitNdp_Outbound_Neighbor_Solicitation();
	static const GUID &Filter_Baseline_PermitNdp_Inbound_Neighbor_Solicitation();
	static const GUID &Filter_Baseline_PermitNdp_Outbound_Neighbor_Advertisement();
	static const GUID &Filter_Baseline_PermitNdp_Inbound_Neighbor_Advertisement();
	static const GUID &Filter_Baseline_PermitNdp_Inbound_Redirect();

	static const GUID &Filter_Baseline_PermitDns_Outbound_Ipv4();
	static const GUID &Filter_Baseline_PermitDns_Outbound_Ipv6();

	static const GUID &Filter_Dns_BlockAll_Outbound_Ipv4();
	static const GUID &Filter_Dns_BlockAll_Outbound_Ipv6();
	static const GUID &Filter_Dns_PermitNonTunnel_Outbound_Ipv4();
	static const GUID &Filter_Dns_PermitNonTunnel_Outbound_Ipv6();
	static const GUID &Filter_Dns_PermitTunnel_Outbound_Ipv4();
	static const GUID &Filter_Dns_PermitTunnel_Outbound_Ipv6();
	static const GUID &Filter_Dns_PermitLoopback_Outbound_Ipv4();
	static const GUID &Filter_Dns_PermitLoopback_Outbound_Ipv6();

	//
	// Persistent and boot-time filters
	//

	static const GUID &ProviderPersistent();
	static const GUID &SublayerPersistent();

	static const GUID &Filter_Boottime_BlockAll_Inbound_Ipv4();
	static const GUID &Filter_Boottime_BlockAll_Outbound_Ipv4();
	static const GUID &Filter_Boottime_BlockAll_Inbound_Ipv6();
	static const GUID &Filter_Boottime_BlockAll_Outbound_Ipv6();

	static const GUID &Filter_Persistent_BlockAll_Inbound_Ipv4();
	static const GUID &Filter_Persistent_BlockAll_Outbound_Ipv4();
	static const GUID &Filter_Persistent_BlockAll_Inbound_Ipv6();
	static const GUID &Filter_Persistent_BlockAll_Outbound_Ipv6();
};
